//! Headless software rasterizer for PNG export (`-o out.png`). Pure Rust, no GPU:
//! it projects the tessellated mesh, z-buffers and flat-shades every triangle,
//! and encodes a PNG. Colors come from the B3 color groups; the fused geometry
//! (STL/oracle) is untouched. Quality is thumbnail-grade with 2× supersampling.

use openrscad_geom::Mesh;

type V3 = [f64; 3];
type M4 = [[f64; 4]; 4];

/// Camera projection.
#[derive(Clone, Copy, Debug)]
pub enum Projection {
    Perspective { fov_deg: f64 },
    Ortho,
}

/// Where the camera is. `Auto` frames the whole model isometrically.
#[derive(Clone, Copy, Debug)]
pub enum Camera {
    Auto,
    /// OpenSCAD gimbal form: look at `target`, Euler-rotate (deg) about the axes,
    /// eye at `dist`. `rot = [0,0,0]` is a top view. Approximates OpenSCAD's
    /// camera (clean-room; reconstructed from the documented flag, not source).
    Gimbal {
        target: V3,
        rot: V3,
        dist: f64,
    },
    /// Exact eye/center form (`lookAt`, up = +Z).
    Eye {
        eye: V3,
        center: V3,
    },
}

pub struct RenderOpts {
    pub width: u32,
    pub height: u32,
    pub camera: Camera,
    pub projection: Projection,
    pub background: [u8; 3],
    /// Frame the whole model regardless of the requested distance (also the
    /// default for `Camera::Auto`).
    pub viewall: bool,
    /// Shift the model so its bbox center is the view target.
    pub autocenter: bool,
}

impl Default for RenderOpts {
    fn default() -> Self {
        RenderOpts {
            width: 512,
            height: 512,
            camera: Camera::Auto,
            projection: Projection::Perspective { fov_deg: 45.0 },
            background: [0x1a, 0x1d, 0x23], // the viewer's background
            viewall: false,
            autocenter: false,
        }
    }
}

// ---- small vector / matrix math ------------------------------------------

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}
fn normalize(a: V3) -> V3 {
    let n = norm(a);
    if n > 1e-12 {
        scale(a, 1.0 / n)
    } else {
        [0.0, 0.0, 0.0]
    }
}

fn mat_mul(a: &M4, b: &M4) -> M4 {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

/// Multiply a 4×4 matrix by `(v, 1)`, returning the homogeneous `[x,y,z,w]`.
fn mat_vec(m: &M4, v: V3) -> [f64; 4] {
    let p = [v[0], v[1], v[2], 1.0];
    let mut o = [0.0; 4];
    for (i, oi) in o.iter_mut().enumerate() {
        *oi = (0..4).map(|k| m[i][k] * p[k]).sum();
    }
    o
}

/// Rotation about X, Y, Z (degrees), applied in Z·Y·X order.
fn euler(rot: V3) -> M4 {
    let (rx, ry, rz) = (
        rot[0].to_radians(),
        rot[1].to_radians(),
        rot[2].to_radians(),
    );
    let (sx, cx) = (rx.sin(), rx.cos());
    let (sy, cy) = (ry.sin(), ry.cos());
    let (sz, cz) = (rz.sin(), rz.cos());
    let mx = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cx, -sx, 0.0],
        [0.0, sx, cx, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let my = [
        [cy, 0.0, sy, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-sy, 0.0, cy, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mz = [
        [cz, -sz, 0.0, 0.0],
        [sz, cz, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    mat_mul(&mz, &mat_mul(&my, &mx))
}

fn rot3(m: &M4, v: V3) -> V3 {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// Right-handed `lookAt` (camera looks down -Z), OpenGL convention.
fn look_at(eye: V3, center: V3, up: V3) -> M4 {
    let f = normalize(sub(center, eye));
    let s = normalize(cross(f, up));
    let u = cross(s, f);
    [
        [s[0], s[1], s[2], -dot(s, eye)],
        [u[0], u[1], u[2], -dot(u, eye)],
        [-f[0], -f[1], -f[2], dot(f, eye)],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn perspective(fov_deg: f64, aspect: f64, near: f64, far: f64) -> M4 {
    let f = 1.0 / (fov_deg.to_radians() * 0.5).tan();
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [
            0.0,
            0.0,
            (far + near) / (near - far),
            (2.0 * far * near) / (near - far),
        ],
        [0.0, 0.0, -1.0, 0.0],
    ]
}

fn ortho(half_w: f64, half_h: f64, near: f64, far: f64) -> M4 {
    [
        [1.0 / half_w, 0.0, 0.0, 0.0],
        [0.0, 1.0 / half_h, 0.0, 0.0],
        [0.0, 0.0, -2.0 / (far - near), -(far + near) / (far - near)],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

// ---- scene framing --------------------------------------------------------

/// Combined bounding box of every mesh (min, max), or None if all empty.
fn combined_bbox(meshes: &[(&Mesh, [f32; 4])]) -> Option<(V3, V3)> {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    let mut any = false;
    for (m, _) in meshes {
        if let Some((l, h)) = m.bbox() {
            any = true;
            for i in 0..3 {
                lo[i] = lo[i].min(l[i]);
                hi[i] = hi[i].max(h[i]);
            }
        }
    }
    any.then_some((lo, hi))
}

// ---- shading --------------------------------------------------------------

/// Flat two-sided Lambert shade of a face with normal `n` and base `color`.
fn shade(n: V3, color: [f32; 4]) -> [f64; 3] {
    let key = normalize([0.4, -0.6, 0.8]); // mirrors the viewer's key light
    let fill = normalize([-0.5, 0.4, 0.2]);
    let ndl = |l: V3| dot(n, l).abs();
    let i = 0.35 + 0.75 * ndl(key) + 0.2 * ndl(fill);
    [
        (color[0] as f64 * i).clamp(0.0, 1.0),
        (color[1] as f64 * i).clamp(0.0, 1.0),
        (color[2] as f64 * i).clamp(0.0, 1.0),
    ]
}

// ---- the rasterizer -------------------------------------------------------

fn edge(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Render the colored meshes to an in-memory PNG (RGB8). `meshes` should already
/// exclude `%` background groups.
pub fn render_png(meshes: &[(&Mesh, [f32; 4])], opts: &RenderOpts) -> Result<Vec<u8>, String> {
    let w = opts.width.max(1);
    let h = opts.height.max(1);
    // 2× supersampling for cleaner edges, then box-downsample.
    let ss = 2u32;
    let (sw, sh) = (w * ss, h * ss);
    let aspect = sw as f64 / sh as f64;

    let (lo, hi) = combined_bbox(meshes).unwrap_or(([0.0; 3], [0.0; 3]));
    let center = scale(add(lo, hi), 0.5);
    let size = sub(hi, lo);
    let max_ext = size[0].max(size[1]).max(size[2]);
    let radius = max_ext * 0.75 + 1.0;

    // Optionally recenter the model at the origin.
    let shift = if opts.autocenter {
        center
    } else {
        [0.0, 0.0, 0.0]
    };
    let scene_center = sub(center, shift);

    let fov = match opts.projection {
        Projection::Perspective { fov_deg } => fov_deg,
        Projection::Ortho => 45.0,
    };

    // Resolve the eye/target/up.
    let (eye, target, up) = match opts.camera {
        Camera::Eye { eye, center } if !opts.viewall => (eye, center, [0.0, 0.0, 1.0]),
        Camera::Gimbal { target, rot, dist } if !opts.viewall => {
            let r = euler(rot);
            let dir = rot3(&r, [0.0, 0.0, 1.0]);
            let up = rot3(&r, [0.0, 1.0, 0.0]);
            (add(target, scale(dir, dist)), target, up)
        }
        // Auto, or any camera under --viewall: frame the whole model isometrically.
        _ => {
            let dir = normalize([0.6, -0.8, 0.5]);
            let dist = radius / (fov.to_radians() * 0.5).sin();
            (
                add(scene_center, scale(dir, dist)),
                scene_center,
                [0.0, 0.0, 1.0],
            )
        }
    };

    let cam_dist = norm(sub(eye, target));
    let near = (cam_dist - radius).max(cam_dist * 0.01).max(1e-3);
    let far = cam_dist + radius * 2.0 + 1.0;
    let proj = match opts.projection {
        Projection::Perspective { fov_deg } => perspective(fov_deg, aspect, near, far),
        Projection::Ortho => {
            let half_h = radius * 1.1;
            ortho(half_h * aspect, half_h, near, far)
        }
    };
    let mvp = mat_mul(&proj, &look_at(eye, target, up));

    let npix = (sw * sh) as usize;
    let mut zbuf = vec![f64::INFINITY; npix];
    let mut color = vec![
        [
            opts.background[0] as f64 / 255.0,
            opts.background[1] as f64 / 255.0,
            opts.background[2] as f64 / 255.0,
        ];
        npix
    ];

    for (mesh, rgba) in meshes {
        let (positions, normals) = mesh.to_triangle_soup_f32();
        let tri_count = positions.len() / 9;
        for t in 0..tri_count {
            let b = t * 9;
            let wp = |k: usize| -> V3 {
                let o = b + k * 3;
                sub(
                    [
                        positions[o] as f64,
                        positions[o + 1] as f64,
                        positions[o + 2] as f64,
                    ],
                    shift,
                )
            };
            let nrm = normalize([
                normals[b] as f64,
                normals[b + 1] as f64,
                normals[b + 2] as f64,
            ]);
            let col = shade(nrm, *rgba);

            // Project the three vertices to screen space (+ depth). Cull the
            // triangle if any vertex is at/behind the near plane.
            let mut sp = [[0.0f64; 3]; 3]; // [x, y, depth]
            let mut ok = true;
            for (k, spk) in sp.iter_mut().enumerate() {
                let clip = mat_vec(&mvp, wp(k));
                if clip[3] <= 1e-6 {
                    ok = false;
                    break;
                }
                let ndc = [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]];
                spk[0] = (ndc[0] * 0.5 + 0.5) * sw as f64;
                spk[1] = (1.0 - (ndc[1] * 0.5 + 0.5)) * sh as f64;
                spk[2] = ndc[2] * 0.5 + 0.5;
            }
            if !ok {
                continue;
            }

            let a2 = [sp[0][0], sp[0][1]];
            let b2 = [sp[1][0], sp[1][1]];
            let c2 = [sp[2][0], sp[2][1]];
            let area = edge(a2, b2, c2);
            if area.abs() < 1e-9 {
                continue;
            }
            let s = area.signum();

            let min_x = a2[0].min(b2[0]).min(c2[0]).floor().max(0.0) as u32;
            let max_x = a2[0].max(b2[0]).max(c2[0]).ceil().min(sw as f64) as u32;
            let min_y = a2[1].min(b2[1]).min(c2[1]).floor().max(0.0) as u32;
            let max_y = a2[1].max(b2[1]).max(c2[1]).ceil().min(sh as f64) as u32;

            for py in min_y..max_y {
                for px in min_x..max_x {
                    let p = [px as f64 + 0.5, py as f64 + 0.5];
                    let w0 = edge(b2, c2, p);
                    let w1 = edge(c2, a2, p);
                    let w2 = edge(a2, b2, p);
                    if w0 * s < 0.0 || w1 * s < 0.0 || w2 * s < 0.0 {
                        continue;
                    }
                    let (l0, l1, l2) = (w0 / area, w1 / area, w2 / area);
                    let depth = l0 * sp[0][2] + l1 * sp[1][2] + l2 * sp[2][2];
                    let idx = (py * sw + px) as usize;
                    if depth < zbuf[idx] {
                        zbuf[idx] = depth;
                        color[idx] = col;
                    }
                }
            }
        }
    }

    // Box-downsample the supersampled buffer to the target size.
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f64; 3];
            for dy in 0..ss {
                for dx in 0..ss {
                    let sx = x * ss + dx;
                    let sy = y * ss + dy;
                    let c = color[(sy * sw + sx) as usize];
                    acc[0] += c[0];
                    acc[1] += c[1];
                    acc[2] += c[2];
                }
            }
            let n = (ss * ss) as f64;
            for v in acc {
                rgb.push((v / n * 255.0).round().clamp(0.0, 255.0) as u8);
            }
        }
    }

    encode_png(&rgb, w, h)
}

fn encode_png(rgb: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(rgb).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrscad_geom::cube;

    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let dec = png::Decoder::new(bytes);
        let mut reader = dec.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        buf.truncate(info.buffer_size());
        (info.width, info.height, buf)
    }

    #[test]
    fn renders_a_cube_png() {
        let mesh = cube([10.0, 10.0, 10.0], true);
        let opts = RenderOpts {
            width: 64,
            height: 48,
            ..Default::default()
        };
        let png = render_png(&[(&mesh, [1.0, 0.0, 0.0, 1.0])], &opts).unwrap();
        let (w, h, pixels) = decode(&png);
        assert_eq!((w, h), (64, 48));

        let bg = [0x1a, 0x1d, 0x23];
        let px = |x: u32, y: u32| {
            let i = ((y * w + x) * 3) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };
        // A corner is background; the centre is the drawn (reddish) cube.
        assert_eq!(px(0, 0), bg);
        let c = px(w / 2, h / 2);
        assert_ne!(c, bg, "centre should be the cube, not background");
        assert!(c[0] > c[1] && c[0] > c[2], "cube should be reddish: {c:?}");
    }

    #[test]
    fn look_at_and_gimbal_math() {
        // Eye form: looking down -Z from above, forward is -Z.
        let v = look_at([0.0, 0.0, 10.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        // The origin maps to (0,0,-10) in camera space (10 in front).
        let o = mat_vec(&v, [0.0, 0.0, 0.0]);
        assert!((o[2] - -10.0).abs() < 1e-9, "z {o:?}");

        // Gimbal rot (0,0,0): eye sits straight above the target (top view).
        let r = euler([0.0, 0.0, 0.0]);
        let dir = rot3(&r, [0.0, 0.0, 1.0]);
        assert!((dir[2] - 1.0).abs() < 1e-9 && dir[0].abs() < 1e-9 && dir[1].abs() < 1e-9);
    }
}
