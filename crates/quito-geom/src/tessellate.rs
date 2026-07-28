//! Primitive tessellation, including the bit-exact fragment formula.
//!
//! Geometry compatibility with OpenSCAD hinges on the fragment count formula
//! and the exact vertex placement of curved primitives, so these are
//! reconstructed carefully from the documented behavior.

use crate::mesh::Mesh;
use quito_ir::FragmentSpec;
use std::f64::consts::PI;

/// OpenSCAD's `GRID_FINE` threshold below which a primitive collapses to the
/// minimum 3 fragments.
const GRID_FINE: f64 = 0.000_000_953_674_316_406_25;

/// Number of fragments in a full circle of radius `r`, given `$fn/$fa/$fs`.
pub fn fragments(r: f64, f: FragmentSpec) -> u32 {
    if r < GRID_FINE || f.fn_.is_nan() || f.fn_.is_infinite() {
        return 3;
    }
    if f.fn_ > 0.0 {
        return if f.fn_ >= 3.0 { f.fn_ as u32 } else { 3 };
    }
    let n = (360.0 / f.fa).min(r * 2.0 * PI / f.fs).max(5.0).ceil();
    n as u32
}

/// A point on a circle of `n` fragments, `i`-th fragment, radius `r`.
fn circle_point(r: f64, i: u32, n: u32) -> [f64; 2] {
    let phi = (2.0 * PI * i as f64) / n as f64;
    [r * phi.cos(), r * phi.sin()]
}

/// Build a mesh from explicit points and (possibly polygonal) faces by
/// fan-triangulating each face. OpenSCAD orders face vertices clockwise as seen
/// from outside; our mesh convention is counter-clockwise (outward normal by
/// the right-hand rule), so each face is reversed during triangulation.
pub fn polyhedron(points: &[[f64; 3]], faces: &[Vec<u32>]) -> Mesh {
    let verts = points.to_vec();
    let mut tris = Vec::new();
    for face in faces {
        if face.len() < 3 {
            continue;
        }
        for k in 1..face.len() - 1 {
            tris.push([face[0], face[k + 1], face[k]]);
        }
    }
    Mesh { verts, tris }
}

/// Axis-aligned box.
pub fn cube(size: [f64; 3], center: bool) -> Mesh {
    let (lo, hi) = if center {
        (
            [-size[0] / 2.0, -size[1] / 2.0, -size[2] / 2.0],
            [size[0] / 2.0, size[1] / 2.0, size[2] / 2.0],
        )
    } else {
        ([0.0, 0.0, 0.0], size)
    };
    let v = |x: bool, y: bool, z: bool| {
        [
            if x { hi[0] } else { lo[0] },
            if y { hi[1] } else { lo[1] },
            if z { hi[2] } else { lo[2] },
        ]
    };
    let verts = vec![
        v(false, false, false), // 0
        v(true, false, false),  // 1
        v(true, true, false),   // 2
        v(false, true, false),  // 3
        v(false, false, true),  // 4
        v(true, false, true),   // 5
        v(true, true, true),    // 6
        v(false, true, true),   // 7
    ];
    // Outward-facing (CCW) triangles.
    let tris = vec![
        [0, 3, 2],
        [0, 2, 1], // bottom (z-)
        [4, 5, 6],
        [4, 6, 7], // top (z+)
        [0, 1, 5],
        [0, 5, 4], // front (y-)
        [2, 3, 7],
        [2, 7, 6], // back (y+)
        [1, 2, 6],
        [1, 6, 5], // right (x+)
        [0, 4, 7],
        [0, 7, 3], // left (x-)
    ];
    let mut m = Mesh { verts, tris };
    m.ensure_outward();
    m
}

/// Sphere, tessellated with OpenSCAD's ring topology (flat poles).
pub fn sphere(r: f64, frags: FragmentSpec) -> Mesh {
    let n = fragments(r, frags).max(3);
    let num_rings = n.div_ceil(2);
    if num_rings == 0 || r <= 0.0 {
        return Mesh::new();
    }

    let mut verts: Vec<[f64; 3]> = Vec::with_capacity((num_rings * n) as usize);
    for i in 0..num_rings {
        let phi = (PI * (i as f64 + 0.5)) / num_rings as f64;
        let ring_r = r * phi.sin();
        let z = r * phi.cos();
        for j in 0..n {
            let p = circle_point(ring_r, j, n);
            verts.push([p[0], p[1], z]);
        }
    }

    let mut tris: Vec<[u32; 3]> = Vec::new();
    let idx = |ring: u32, j: u32| ring * n + (j % n);

    // side bands
    for i in 0..num_rings - 1 {
        for j in 0..n {
            let v00 = idx(i, j);
            let v01 = idx(i, j + 1);
            let v10 = idx(i + 1, j);
            let v11 = idx(i + 1, j + 1);
            tris.push([v00, v10, v11]);
            tris.push([v00, v11, v01]);
        }
    }

    // top cap (ring 0, near +z) — fan
    for j in 1..n - 1 {
        tris.push([idx(0, 0), idx(0, j), idx(0, j + 1)]);
    }
    // bottom cap (last ring, near -z) — fan (reversed)
    let last = num_rings - 1;
    for j in 1..n - 1 {
        tris.push([idx(last, 0), idx(last, j + 1), idx(last, j)]);
    }

    let mut m = Mesh { verts, tris };
    m.ensure_outward();
    m
}

/// Cylinder / cone / frustum along +Z.
pub fn cylinder(h: f64, r1: f64, r2: f64, center: bool, frags: FragmentSpec) -> Mesh {
    let n = fragments(r1.max(r2), frags).max(3);
    let (z0, z1) = if center {
        (-h / 2.0, h / 2.0)
    } else {
        (0.0, h)
    };

    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();

    let bottom_apex = r1 <= 0.0;
    let top_apex = r2 <= 0.0;

    // Both ends collapsed -> nothing.
    if bottom_apex && top_apex {
        return Mesh::new();
    }

    // Bottom vertices.
    let bottom_start = verts.len() as u32;
    if bottom_apex {
        verts.push([0.0, 0.0, z0]);
    } else {
        for j in 0..n {
            let p = circle_point(r1, j, n);
            verts.push([p[0], p[1], z0]);
        }
    }
    // Top vertices.
    let top_start = verts.len() as u32;
    if top_apex {
        verts.push([0.0, 0.0, z1]);
    } else {
        for j in 0..n {
            let p = circle_point(r2, j, n);
            verts.push([p[0], p[1], z1]);
        }
    }

    let b = |j: u32| bottom_start + (j % n);
    let t = |j: u32| top_start + (j % n);

    // Side walls.
    if bottom_apex {
        let apex = bottom_start;
        for j in 0..n {
            tris.push([apex, t(j + 1), t(j)]);
        }
    } else if top_apex {
        let apex = top_start;
        for j in 0..n {
            tris.push([b(j), b(j + 1), apex]);
        }
    } else {
        for j in 0..n {
            let b0 = b(j);
            let b1 = b(j + 1);
            let t0 = t(j);
            let t1 = t(j + 1);
            tris.push([b0, b1, t1]);
            tris.push([b0, t1, t0]);
        }
    }

    // Bottom cap (facing -Z): fan reversed.
    if !bottom_apex {
        for j in 1..n - 1 {
            tris.push([b(0), b(j + 1), b(j)]);
        }
    }
    // Top cap (facing +Z): fan.
    if !top_apex {
        for j in 1..n - 1 {
            tris.push([t(0), t(j), t(j + 1)]);
        }
    }

    let mut m = Mesh { verts, tris };
    m.ensure_outward();
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(fn_: f64) -> FragmentSpec {
        FragmentSpec {
            fn_,
            fa: 12.0,
            fs: 2.0,
        }
    }

    #[test]
    fn fragment_formula() {
        // $fn wins when >= 3
        assert_eq!(fragments(10.0, spec(6.0)), 6);
        assert_eq!(fragments(10.0, spec(1.0)), 3);
        // default $fa=12, $fs=2: for r=10 -> min(30, 31.4) -> 30
        assert_eq!(fragments(10.0, spec(0.0)), 30);
        // tiny radius collapses to 3
        assert_eq!(fragments(1e-9, spec(0.0)), 3);
        // minimum of 5
        assert_eq!(fragments(0.1, spec(0.0)), 5);
    }

    #[test]
    fn cube_volume_and_outward() {
        let m = cube([2.0, 3.0, 4.0], false);
        assert_eq!(m.tris.len(), 12);
        assert!((m.volume() - 24.0).abs() < 1e-9);
        assert!(m.signed_volume() > 0.0, "cube must be outward-facing");
    }

    #[test]
    fn sphere_outward_and_approx_volume() {
        let m = sphere(10.0, spec(64.0));
        assert!(m.signed_volume() > 0.0, "sphere must be outward-facing");
        let analytic = 4.0 / 3.0 * PI * 1000.0;
        // faceted sphere under-approximates; within ~2%.
        let rel = (m.volume() - analytic).abs() / analytic;
        assert!(rel < 0.02, "sphere volume off by {rel}");
    }

    #[test]
    fn cylinder_outward_and_volume() {
        let m = cylinder(10.0, 5.0, 5.0, false, spec(128.0));
        assert!(m.signed_volume() > 0.0);
        let analytic = PI * 25.0 * 10.0;
        let rel = (m.volume() - analytic).abs() / analytic;
        assert!(rel < 0.01, "cylinder volume off by {rel}");
    }

    #[test]
    fn polyhedron_fan_triangulation() {
        let pts = vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]];
        let faces = vec![vec![0u32, 1, 2, 3]]; // one quad -> 2 triangles
        let m = polyhedron(&pts, &faces);
        assert_eq!(m.tris.len(), 2);
        assert_eq!(m.verts.len(), 4);
    }

    #[test]
    fn cone_is_manifold_shape() {
        let m = cylinder(10.0, 5.0, 0.0, false, spec(32.0));
        assert!(m.signed_volume() > 0.0);
        assert!(!m.verts.is_empty());
    }
}
