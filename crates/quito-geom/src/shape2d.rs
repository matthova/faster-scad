//! 2D shapes (contours), polygon triangulation, and 2D→3D extrusion.
//!
//! A 2D node renders to a set of closed contours (`Vec<Contour>`); the first is
//! the outer boundary, any others are treated as holes. Extrusions turn these
//! into a mesh. 2D boolean ops (difference/intersection/offset/hull) are not
//! implemented yet — unions concatenate contours.

use crate::mesh::Mesh;
use crate::tessellate::fragments;
use quito_ir::{FragmentSpec, Node};
use std::f64::consts::PI;

pub type Point2 = [f64; 2];
pub type Contour = Vec<Point2>;

/// Render a 2D subtree to contours.
pub fn render2d(node: &Node) -> Vec<Contour> {
    match node {
        Node::Empty => Vec::new(),
        Node::Square { size, center } => vec![square_contour(*size, *center)],
        Node::Circle { r, frags } => vec![circle_contour(*r, *frags)],
        Node::Polygon { points, paths } => polygon_contours(points, paths),
        Node::Translate { v, child } => {
            map_contours(render2d(child), |p| [p[0] + v[0], p[1] + v[1]])
        }
        Node::Scale { v, child } => map_contours(render2d(child), |p| [p[0] * v[0], p[1] * v[1]]),
        Node::Rotate { deg, child } => {
            let a = deg[2].to_radians();
            let (s, c) = (a.sin(), a.cos());
            map_contours(render2d(child), |p| [p[0] * c - p[1] * s, p[0] * s + p[1] * c])
        }
        // Union/group: concatenate contours (no clipping yet).
        Node::Group(children) | Node::Union(children) => {
            children.iter().flat_map(|c| render2d(c)).collect()
        }
        _ => Vec::new(),
    }
}

fn map_contours(cs: Vec<Contour>, f: impl Fn(Point2) -> Point2) -> Vec<Contour> {
    cs.into_iter().map(|c| c.into_iter().map(&f).collect()).collect()
}

fn square_contour(size: Point2, center: bool) -> Contour {
    let (x0, y0) = if center {
        (-size[0] / 2.0, -size[1] / 2.0)
    } else {
        (0.0, 0.0)
    };
    let (x1, y1) = (x0 + size[0], y0 + size[1]);
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]] // CCW
}

fn circle_contour(r: f64, frags: FragmentSpec) -> Contour {
    let n = fragments(r, frags).max(3);
    (0..n)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / n as f64;
            [r * a.cos(), r * a.sin()]
        })
        .collect()
}

fn polygon_contours(points: &[Point2], paths: &Option<Vec<Vec<u32>>>) -> Vec<Contour> {
    match paths {
        Some(paths) => paths
            .iter()
            .map(|path| path.iter().map(|&i| points[i as usize]).collect())
            .collect(),
        None => vec![points.to_vec()],
    }
}

/// Signed area of a contour (positive when counter-clockwise).
fn signed_area(c: &[Point2]) -> f64 {
    let mut a = 0.0;
    for i in 0..c.len() {
        let p = c[i];
        let q = c[(i + 1) % c.len()];
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

/// Ear-clipping triangulation of a single simple polygon. Returns index
/// triples into `poly`. Assumes no holes; input is made CCW.
fn triangulate_simple(poly: &[Point2]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    // Work on an index list, CCW.
    let mut idx: Vec<usize> = (0..n).collect();
    if signed_area(poly) < 0.0 {
        idx.reverse();
    }

    let cross = |o: Point2, a: Point2, b: Point2| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let in_tri = |p: Point2, a: Point2, b: Point2, c: Point2| {
        let d1 = cross(a, b, p);
        let d2 = cross(b, c, p);
        let d3 = cross(c, a, p);
        let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(neg && pos)
    };

    let mut tris = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 {
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let ia = idx[(i + m - 1) % m];
            let ib = idx[i];
            let ic = idx[(i + 1) % m];
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if cross(a, b, c) <= 0.0 {
                continue; // reflex or degenerate
            }
            // no other vertex inside this ear
            let mut ear = true;
            for &j in &idx {
                if j == ia || j == ib || j == ic {
                    continue;
                }
                if in_tri(poly[j], a, b, c) {
                    ear = false;
                    break;
                }
            }
            if ear {
                tris.push([ia, ib, ic]);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        guard += 1;
        if !clipped || guard > n + 5 {
            break; // degenerate; stop
        }
    }
    if idx.len() == 3 {
        tris.push([idx[0], idx[1], idx[2]]);
    }
    tris
}

/// A flat mesh of a 2D shape at z=0 (used when a 2D node is the render target).
pub fn flat_mesh(contours: &[Contour]) -> Mesh {
    let mut mesh = Mesh::new();
    for c in contours {
        let base = mesh.verts.len() as u32;
        for p in c {
            mesh.verts.push([p[0], p[1], 0.0]);
        }
        for t in triangulate_simple(c) {
            mesh.tris.push([base + t[0] as u32, base + t[1] as u32, base + t[2] as u32]);
        }
    }
    mesh
}

/// `linear_extrude` of the contours to a mesh.
pub fn linear_extrude(
    contours: &[Contour],
    height: f64,
    center: bool,
    twist: f64,
    scale: Point2,
    slices: u32,
) -> Mesh {
    let mut mesh = Mesh::new();
    let z0 = if center { -height / 2.0 } else { 0.0 };
    for c in contours {
        extrude_one(&mut mesh, c, z0, height, twist, scale, slices.max(1));
    }
    mesh.ensure_outward();
    mesh
}

fn extrude_one(
    mesh: &mut Mesh,
    contour: &[Point2],
    z0: f64,
    height: f64,
    twist: f64,
    scale: Point2,
    slices: u32,
) {
    let n = contour.len();
    if n < 3 {
        return;
    }
    let base = mesh.verts.len() as u32;
    // Build `slices+1` rings.
    for layer in 0..=slices {
        let t = layer as f64 / slices as f64;
        let ang = (-twist * t).to_radians();
        let (s, c) = (ang.sin(), ang.cos());
        let sx = 1.0 + (scale[0] - 1.0) * t;
        let sy = 1.0 + (scale[1] - 1.0) * t;
        let z = z0 + height * t;
        for p in contour {
            let (x, y) = (p[0] * sx, p[1] * sy);
            mesh.verts.push([x * c - y * s, x * s + y * c, z]);
        }
    }
    let ring = |layer: u32, i: usize| base + layer * n as u32 + i as u32;
    // Walls.
    for layer in 0..slices {
        for i in 0..n {
            let j = (i + 1) % n;
            let a = ring(layer, i);
            let b = ring(layer, j);
            let cc = ring(layer + 1, j);
            let d = ring(layer + 1, i);
            mesh.tris.push([a, b, cc]);
            mesh.tris.push([a, cc, d]);
        }
    }
    // Caps: bottom (layer 0, facing -z) and top (last layer, facing +z).
    let tris = triangulate_simple(contour);
    for tri in &tris {
        // bottom reversed
        mesh.tris.push([ring(0, tri[0]), ring(0, tri[2]), ring(0, tri[1])]);
        // top
        mesh.tris.push([
            ring(slices, tri[0]),
            ring(slices, tri[1]),
            ring(slices, tri[2]),
        ]);
    }
}

/// `rotate_extrude` of the contours around the Z axis.
pub fn rotate_extrude(contours: &[Contour], angle: f64, frags: FragmentSpec) -> Mesh {
    let mut mesh = Mesh::new();
    let max_r = contours
        .iter()
        .flat_map(|c| c.iter())
        .map(|p| p[0])
        .fold(0.0_f64, f64::max);
    let full = (angle.abs() - 360.0).abs() < 1e-9 || angle.abs() >= 360.0;
    let steps = fragments(max_r, frags).max(3);
    for c in contours {
        revolve_one(&mut mesh, c, angle, steps, full);
    }
    mesh.ensure_outward();
    mesh
}

fn revolve_one(mesh: &mut Mesh, contour: &[Point2], angle: f64, steps: u32, full: bool) {
    let n = contour.len();
    if n < 3 {
        return;
    }
    let base = mesh.verts.len() as u32;
    let ring_count = if full { steps } else { steps + 1 };
    for k in 0..ring_count {
        let frac = if full {
            k as f64 / steps as f64
        } else {
            k as f64 / steps as f64
        };
        let th = (angle * frac).to_radians();
        let (s, c) = (th.sin(), th.cos());
        for p in contour {
            // 2D point (x=radius, y=height) -> 3D ring.
            mesh.verts.push([p[0] * c, p[0] * s, p[1]]);
        }
    }
    let ring = |k: u32, i: usize| base + (k % ring_count) * n as u32 + i as u32;
    let wall_steps = if full { steps } else { steps };
    for k in 0..wall_steps {
        for i in 0..n {
            let j = (i + 1) % n;
            let a = ring(k, i);
            let b = ring(k, j);
            let cc = ring(k + 1, j);
            let d = ring(k + 1, i);
            mesh.tris.push([a, b, cc]);
            mesh.tris.push([a, cc, d]);
        }
    }
    // End caps for a partial sweep.
    if !full {
        let tris = triangulate_simple(contour);
        for tri in &tris {
            mesh.tris.push([ring(0, tri[0]), ring(0, tri[2]), ring(0, tri[1])]);
            mesh.tris.push([
                ring(steps, tri[0]),
                ring(steps, tri[1]),
                ring(steps, tri[2]),
            ]);
        }
    }
}
