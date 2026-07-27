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
        Node::Offset { r, delta, chamfer, frags, child } => {
            offset(&render2d(child), *r, *delta, *chamfer, *frags)
        }
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

/// Cross-section of a mesh at the z=0 plane (`projection(cut=true)`): returns
/// the closed contours where the mesh crosses the plane.
pub fn slice_z0(mesh: &Mesh) -> Vec<Contour> {
    // Slice slightly above 0 to avoid coplanar-face degeneracies.
    const Z: f64 = 1e-7;
    let mut segs: Vec<(Point2, Point2)> = Vec::new();
    for t in &mesh.tris {
        let v = [
            mesh.verts[t[0] as usize],
            mesh.verts[t[1] as usize],
            mesh.verts[t[2] as usize],
        ];
        let mut cross = Vec::new();
        for &(a, b) in &[(0, 1), (1, 2), (2, 0)] {
            let (za, zb) = (v[a][2] - Z, v[b][2] - Z);
            if (za < 0.0) != (zb < 0.0) {
                let f = za / (za - zb);
                cross.push([
                    v[a][0] + (v[b][0] - v[a][0]) * f,
                    v[a][1] + (v[b][1] - v[a][1]) * f,
                ]);
            }
        }
        if cross.len() == 2 {
            segs.push((cross[0], cross[1]));
        }
    }
    chain_segments(segs)
}

/// Chain unordered segments into closed contours by walking segment by segment
/// (so points shared by collinear segments are handled correctly).
fn chain_segments(segs: Vec<(Point2, Point2)>) -> Vec<Contour> {
    let key = |p: Point2| [(p[0] * 1e5).round() as i64, (p[1] * 1e5).round() as i64];
    // point key -> indices of incident segments
    let mut inc: std::collections::HashMap<[i64; 2], Vec<usize>> = Default::default();
    for (i, (a, b)) in segs.iter().enumerate() {
        inc.entry(key(*a)).or_default().push(i);
        inc.entry(key(*b)).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut contours = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        let mut contour = Vec::new();
        let mut si = start;
        let mut cur = segs[si].0;
        let start_key = key(cur);
        loop {
            used[si] = true;
            contour.push(cur);
            // step to the other endpoint of the current segment
            cur = if key(segs[si].0) == key(cur) {
                segs[si].1
            } else {
                segs[si].0
            };
            if key(cur) == start_key {
                break; // closed loop
            }
            // next unused segment incident to `cur`
            match inc.get(&key(cur)).and_then(|v| v.iter().find(|&&j| !used[j]).copied()) {
                Some(j) => si = j,
                None => break,
            }
        }
        if contour.len() >= 3 {
            contours.push(contour);
        }
    }
    contours
}

/// 2D offset of contours. `r` rounds convex corners; `delta` mitres (or
/// chamfers). Positive grows, negative shrinks. Works on simple contours; it
/// does not clip self-intersections from large concave offsets (a 2D-clipper
/// refinement).
pub fn offset(contours: &[Contour], r: f64, delta: f64, chamfer: bool, frags: FragmentSpec) -> Vec<Contour> {
    let (amt, rounded) = if r != 0.0 { (r, true) } else { (delta, false) };
    contours
        .iter()
        .filter(|c| c.len() >= 3)
        .map(|c| offset_one(c, amt, rounded, chamfer, frags))
        .collect()
}

fn offset_one(contour: &[Point2], amt: f64, rounded: bool, chamfer: bool, frags: FragmentSpec) -> Contour {
    // Work CCW; reverse the result if the input was CW.
    let cw = signed_area(contour) < 0.0;
    let poly: Vec<Point2> = if cw {
        contour.iter().rev().cloned().collect()
    } else {
        contour.to_vec()
    };
    let n = poly.len();
    let seg_full = fragments(amt.abs(), frags).max(3) as f64;

    // Outward unit normal of edge i (poly[i]->poly[i+1]) for a CCW polygon.
    let edge_normal = |i: usize| -> Point2 {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let d = [b[0] - a[0], b[1] - a[1]];
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-12);
        [d[1] / len, -d[0] / len]
    };

    let mut out: Contour = Vec::new();
    for i in 0..n {
        let vi = poly[i];
        let n_in = edge_normal((i + n - 1) % n);
        let n_out = edge_normal(i);
        let p_in = [vi[0] + amt * n_in[0], vi[1] + amt * n_in[1]];
        let p_out = [vi[0] + amt * n_out[0], vi[1] + amt * n_out[1]];

        // Convex (left turn) for a CCW polygon.
        let din = [
            vi[0] - poly[(i + n - 1) % n][0],
            vi[1] - poly[(i + n - 1) % n][1],
        ];
        let dout = [poly[(i + 1) % n][0] - vi[0], poly[(i + 1) % n][1] - vi[1]];
        let turn = din[0] * dout[1] - din[1] * dout[0];
        let convex = turn > 0.0;

        // A corner needs "filling" on its outer side: convex when growing,
        // reflex when shrinking.
        let fill = (amt > 0.0 && convex) || (amt < 0.0 && !convex);
        if fill && rounded {
            let a0 = n_in[1].atan2(n_in[0]);
            let a1 = n_out[1].atan2(n_out[0]);
            let mut da = a1 - a0;
            while da <= -std::f64::consts::PI {
                da += 2.0 * std::f64::consts::PI;
            }
            while da > std::f64::consts::PI {
                da -= 2.0 * std::f64::consts::PI;
            }
            let steps = ((seg_full * (da.abs() / (2.0 * std::f64::consts::PI))).ceil() as usize).max(1);
            for s in 0..=steps {
                let a = a0 + da * (s as f64 / steps as f64);
                out.push([vi[0] + amt * a.cos(), vi[1] + amt * a.sin()]);
            }
        } else if fill && chamfer {
            out.push(p_in);
            out.push(p_out);
        } else {
            // miter: intersection of the two offset lines
            match line_intersect(p_in, din, p_out, dout) {
                Some(p) => out.push(p),
                None => {
                    out.push(p_in);
                    out.push(p_out);
                }
            }
        }
    }
    if cw {
        out.reverse();
    }
    out
}

/// Intersection of line (p1, dir d1) with line (p2, dir d2).
fn line_intersect(p1: Point2, d1: Point2, p2: Point2, d2: Point2) -> Option<Point2> {
    let denom = d1[0] * d2[1] - d1[1] * d2[0];
    if denom.abs() < 1e-12 {
        return None;
    }
    let t = ((p2[0] - p1[0]) * d2[1] - (p2[1] - p1[1]) * d2[0]) / denom;
    Some([p1[0] + t * d1[0], p1[1] + t * d1[1]])
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
    if contour.len() < 3 {
        return;
    }
    // Work CCW so walls and caps are consistently outward.
    let owned: Vec<Point2>;
    let contour: &[Point2] = if signed_area(contour) < 0.0 {
        owned = contour.iter().rev().cloned().collect();
        &owned
    } else {
        contour
    };
    let n = contour.len();
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
    if contour.len() < 3 {
        return;
    }
    let owned: Vec<Point2>;
    let contour: &[Point2] = if signed_area(contour) < 0.0 {
        owned = contour.iter().rev().cloned().collect();
        &owned
    } else {
        contour
    };
    let n = contour.len();
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
