//! Pure-Rust 3D convex hull (incremental) for the wasm kernel path. The native
//! kernel uses the C++ Manifold hull, which is more robust; this covers the
//! common cases (hulls of primitive vertex clouds) on wasm.

use crate::mesh::{cross, norm, sub, Mesh};

type P = [f64; 3];

fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Convex hull of a point cloud.
pub fn convex_hull(pts: &[P]) -> Mesh {
    if pts.len() < 4 {
        return Mesh::new();
    }
    let n = pts.len();

    // --- initial tetrahedron ---
    // Two most-separated points along the largest-spread axis.
    let (mut i0, mut i1) = (0usize, 0usize);
    let mut best = -1.0;
    // extremes along each axis give good starting candidates
    let mut ext = Vec::new();
    for axis in 0..3 {
        let mut lo = 0;
        let mut hi = 0;
        for i in 1..n {
            if pts[i][axis] < pts[lo][axis] {
                lo = i;
            }
            if pts[i][axis] > pts[hi][axis] {
                hi = i;
            }
        }
        ext.push(lo);
        ext.push(hi);
    }
    for &a in &ext {
        for &b in &ext {
            let d = norm(sub(pts[a], pts[b]));
            if d > best {
                best = d;
                i0 = a;
                i1 = b;
            }
        }
    }
    if best <= 1e-12 {
        return Mesh::new();
    }
    // farthest point from line i0-i1
    let line = sub(pts[i1], pts[i0]);
    let mut i2 = usize::MAX;
    let mut bestd = 1e-9;
    for i in 0..n {
        let d = norm(cross(line, sub(pts[i], pts[i0])));
        if d > bestd {
            bestd = d;
            i2 = i;
        }
    }
    if i2 == usize::MAX {
        return Mesh::new();
    }
    // farthest point from plane (i0,i1,i2)
    let nrm = cross(sub(pts[i1], pts[i0]), sub(pts[i2], pts[i0]));
    let mut i3 = usize::MAX;
    let mut bestd = 1e-9;
    for i in 0..n {
        let d = dot(nrm, sub(pts[i], pts[i0])).abs();
        if d > bestd {
            bestd = d;
            i3 = i;
        }
    }
    if i3 == usize::MAX {
        return Mesh::new();
    }

    // Oriented faces (outward normals) of the tetra.
    let centroid = [
        (pts[i0][0] + pts[i1][0] + pts[i2][0] + pts[i3][0]) / 4.0,
        (pts[i0][1] + pts[i1][1] + pts[i2][1] + pts[i3][1]) / 4.0,
        (pts[i0][2] + pts[i1][2] + pts[i2][2] + pts[i3][2]) / 4.0,
    ];
    let mut faces: Vec<[usize; 3]> = Vec::new();
    for f in [[i0, i1, i2], [i0, i1, i3], [i0, i2, i3], [i1, i2, i3]] {
        faces.push(orient(f, centroid, pts));
    }

    // --- incremental insertion ---
    let eps = best * 1e-9;
    for p in 0..n {
        if p == i0 || p == i1 || p == i2 || p == i3 {
            continue;
        }
        let pt = pts[p];
        let visible: Vec<usize> = faces
            .iter()
            .enumerate()
            .filter(|(_, f)| signed_dist(**f, pt, pts) > eps)
            .map(|(i, _)| i)
            .collect();
        if visible.is_empty() {
            continue;
        }
        // Horizon = directed edges of visible faces whose reverse isn't also
        // in a visible face.
        let mut edges: std::collections::HashSet<(usize, usize)> = Default::default();
        for &fi in &visible {
            let f = faces[fi];
            for &(a, b) in &[(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                edges.insert((a, b));
            }
        }
        let horizon: Vec<(usize, usize)> = edges
            .iter()
            .filter(|(a, b)| !edges.contains(&(*b, *a)))
            .cloned()
            .collect();
        // Remove visible faces (high indices first).
        let mut vis_sorted = visible.clone();
        vis_sorted.sort_unstable_by(|a, b| b.cmp(a));
        for fi in vis_sorted {
            faces.swap_remove(fi);
        }
        // Add new faces from p to each horizon edge (already outward-oriented).
        for (a, b) in horizon {
            faces.push([a, b, p]);
        }
    }

    Mesh {
        verts: pts.to_vec(),
        tris: faces.iter().map(|f| [f[0] as u32, f[1] as u32, f[2] as u32]).collect(),
    }
}

fn orient(f: [usize; 3], interior: P, pts: &[P]) -> [usize; 3] {
    if signed_dist(f, interior, pts) > 0.0 {
        [f[0], f[2], f[1]] // flip so interior is below
    } else {
        f
    }
}

fn signed_dist(f: [usize; 3], p: P, pts: &[P]) -> f64 {
    let nrm = cross(sub(pts[f[1]], pts[f[0]]), sub(pts[f[2]], pts[f[0]]));
    dot(nrm, sub(p, pts[f[0]])) / norm(nrm).max(1e-12)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_of_cube_corners() {
        let mut pts = Vec::new();
        for x in [0.0, 2.0] {
            for y in [0.0, 2.0] {
                for z in [0.0, 2.0] {
                    pts.push([x, y, z]);
                }
            }
        }
        // add an interior point that must be excluded
        pts.push([1.0, 1.0, 1.0]);
        let m = convex_hull(&pts);
        assert!((m.volume() - 8.0).abs() < 1e-6, "hull vol {}", m.volume());
        assert!(m.signed_volume() > 0.0);
    }
}
