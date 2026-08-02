//! 2D shapes (contours), polygon triangulation, 2D boolean ops, and 2D→3D
//! extrusion.
//!
//! A 2D node renders to a set of closed contours (`Vec<Contour>`) using even-odd
//! nesting (outers + holes). Boolean ops (union/difference/intersection) go
//! through the `geo` polygon clipper via [`boolean_2d`]; projection silhouettes
//! via [`silhouette`].

use crate::mesh::Mesh;
use crate::tessellate::fragments;
use geo::{BooleanOps, LineString, MultiPolygon, Polygon};
use quito_ir::{FragmentSpec, Node};
use std::f64::consts::PI;

pub type Point2 = [f64; 2];
pub type Contour = Vec<Point2>;

/// A 2D boolean operation. (Union goes through [`union_all`], which handles the
/// n-ary case directly, so it is not represented here.)
#[derive(Clone, Copy)]
pub enum Bop {
    Difference,
    Intersection,
}

// ---- contour <-> geo conversions -----------------------------------------

fn to_linestring(c: &Contour) -> LineString<f64> {
    LineString::from(c.iter().map(|p| (p[0], p[1])).collect::<Vec<_>>())
}

fn from_linestring(ls: &LineString<f64>) -> Contour {
    let mut c: Contour = ls.0.iter().map(|p| [p.x, p.y]).collect();
    if c.len() > 1 && c.first() == c.last() {
        c.pop(); // geo rings are closed; drop the duplicate
    }
    c
}

/// Group flat even-odd contours into `(outer, holes)` polygons (outers oriented
/// CCW, holes CW) — the model the clipper needs.
fn group_contours(contours: &[Contour]) -> Vec<(Contour, Vec<Contour>)> {
    let valid: Vec<&Contour> = contours.iter().filter(|c| c.len() >= 3).collect();
    let n = valid.len();
    if n == 0 {
        return Vec::new();
    }
    let rep: Vec<Point2> = valid.iter().map(|c| c[0]).collect();
    let depth: Vec<usize> = (0..n)
        .map(|i| {
            (0..n)
                .filter(|&j| j != i && point_in_polygon(valid[j], rep[i]))
                .count()
        })
        .collect();
    let orient = |c: &Contour, ccw: bool| -> Contour {
        if (signed_area(c) > 0.0) == ccw {
            c.clone()
        } else {
            c.iter().rev().cloned().collect()
        }
    };
    let mut groups: Vec<(Contour, Vec<Contour>)> = Vec::new();
    let mut group_of = vec![None; n];
    for i in 0..n {
        if depth[i] % 2 == 0 {
            group_of[i] = Some(groups.len());
            groups.push((orient(valid[i], true), Vec::new()));
        }
    }
    for h in 0..n {
        if depth[h] % 2 == 1 {
            if let Some(p) =
                (0..n).find(|&p| depth[p] + 1 == depth[h] && point_in_polygon(valid[p], rep[h]))
            {
                if let Some(g) = group_of[p] {
                    groups[g].1.push(orient(valid[h], false));
                }
            }
        }
    }
    groups
}

fn to_multipolygon(contours: &[Contour]) -> MultiPolygon<f64> {
    let polys = group_contours(contours)
        .into_iter()
        .map(|(outer, holes)| {
            Polygon::new(
                to_linestring(&outer),
                holes.iter().map(to_linestring).collect(),
            )
        })
        .collect();
    MultiPolygon::new(polys)
}

fn from_multipolygon(mp: MultiPolygon<f64>) -> Vec<Contour> {
    let mut out = Vec::new();
    for poly in mp {
        out.push(from_linestring(poly.exterior()));
        for hole in poly.interiors() {
            out.push(from_linestring(hole));
        }
    }
    out.retain(|c| c.len() >= 3);
    out
}

// ---- 2D boolean ops -------------------------------------------------------

/// Apply a boolean op between two contour sets, returning result contours
/// (outers + holes, even-odd).
pub fn boolean_2d(a: &[Contour], b: &[Contour], op: Bop) -> Vec<Contour> {
    let (ma, mb) = (to_multipolygon(a), to_multipolygon(b));
    let r = match op {
        Bop::Difference => ma.difference(&mb),
        Bop::Intersection => ma.intersection(&mb),
    };
    from_multipolygon(r)
}

/// Balanced (divide-and-conquer) union of many multipolygons — far faster than a
/// linear fold when unioning e.g. every triangle of a projection.
fn union_multi(mut items: Vec<MultiPolygon<f64>>) -> Option<MultiPolygon<f64>> {
    if items.is_empty() {
        return None;
    }
    while items.len() > 1 {
        let mut next = Vec::with_capacity(items.len().div_ceil(2));
        let mut it = items.into_iter();
        while let Some(a) = it.next() {
            match it.next() {
                Some(b) => next.push(a.union(&b)),
                None => next.push(a),
            }
        }
        items = next;
    }
    items.into_iter().next()
}

/// Union of several contour sets (e.g. the children of a 2D `union`).
pub fn union_all(sets: &[Vec<Contour>]) -> Vec<Contour> {
    let mps: Vec<_> = sets
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| to_multipolygon(s))
        .collect();
    union_multi(mps).map(from_multipolygon).unwrap_or_default()
}

/// The z=0 silhouette of a mesh (`projection(cut=false)`): union every triangle
/// projected onto the XY plane.
pub fn silhouette(mesh: &Mesh) -> Vec<Contour> {
    let mut polys = Vec::new();
    for t in &mesh.tris {
        let a = mesh.verts[t[0] as usize];
        let b = mesh.verts[t[1] as usize];
        let c = mesh.verts[t[2] as usize];
        let area = ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs();
        if area < 1e-9 {
            continue; // edge-on triangle projects to a line
        }
        polys.push(MultiPolygon::new(vec![Polygon::new(
            LineString::from(vec![(a[0], a[1]), (b[0], b[1]), (c[0], c[1])]),
            vec![],
        )]));
    }
    union_multi(polys)
        .map(from_multipolygon)
        .unwrap_or_default()
}

/// 2D convex hull (Andrew's monotone chain) of a point set → one CCW contour.
pub fn hull_2d(mut pts: Vec<Point2>) -> Vec<Contour> {
    pts.sort_by(|a, b| {
        a[0].partial_cmp(&b[0])
            .unwrap()
            .then(a[1].partial_cmp(&b[1]).unwrap())
    });
    pts.dedup();
    if pts.len() < 3 {
        return Vec::new();
    }
    let cross = |o: Point2, a: Point2, b: Point2| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut lower: Vec<Point2> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<Point2> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    vec![lower]
}

/// Render a 2D subtree to contours.
pub fn render2d(node: &Node) -> Vec<Contour> {
    match node {
        Node::Empty => Vec::new(),
        Node::Square { size, center } => vec![square_contour(*size, *center)],
        Node::Circle { r, frags } => vec![circle_contour(*r, *frags)],
        Node::Polygon { points, paths } => polygon_contours(points, paths),
        Node::Import { data, format } => match format.as_str() {
            "dxf" => crate::vector2d::import_dxf(data),
            "svg" => crate::vector2d::import_svg(data),
            _ => Vec::new(),
        },
        Node::Offset {
            r,
            delta,
            chamfer,
            frags,
            child,
        } => offset(&render2d(child), *r, *delta, *chamfer, *frags),
        Node::Translate { v, child } => {
            map_contours(render2d(child), |p| [p[0] + v[0], p[1] + v[1]])
        }
        Node::Scale { v, child } => map_contours(render2d(child), |p| [p[0] * v[0], p[1] * v[1]]),
        Node::Rotate { deg, child } => {
            let a = deg[2].to_radians();
            let (s, c) = (a.sin(), a.cos());
            map_contours(render2d(child), |p| {
                [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
            })
        }
        // 2D reflection across the line through the origin with normal (v.x, v.y).
        Node::Mirror { v, child } => {
            let d = v[0] * v[0] + v[1] * v[1];
            if d == 0.0 {
                render2d(child)
            } else {
                map_contours(render2d(child), |p| {
                    let t = 2.0 * (p[0] * v[0] + p[1] * v[1]) / d;
                    [p[0] - t * v[0], p[1] - t * v[1]]
                })
            }
        }
        // 2D affine: the top-left 2×2 plus the translation column.
        Node::MultMatrix { m, child } => map_contours(render2d(child), |p| {
            [
                m[0][0] * p[0] + m[0][1] * p[1] + m[0][3],
                m[1][0] * p[0] + m[1][1] * p[1] + m[1][3],
            ]
        }),
        Node::Resize { new, auto, child } => resize2d(render2d(child), *new, *auto),
        // Union/group: clip overlaps (proper 2D union).
        Node::Group(children) | Node::Union(children) => {
            let sets: Vec<Vec<Contour>> = children.iter().map(render2d).collect();
            union_all(&sets)
        }
        Node::Difference(children) => {
            let Some((first, rest)) = children.split_first() else {
                return Vec::new();
            };
            let a = render2d(first);
            if rest.is_empty() {
                return a;
            }
            let subtract = union_all(&rest.iter().map(render2d).collect::<Vec<_>>());
            boolean_2d(&a, &subtract, Bop::Difference)
        }
        Node::Intersection(children) => {
            let mut it = children.iter().map(render2d);
            let Some(mut acc) = it.next() else {
                return Vec::new();
            };
            for c in it {
                acc = boolean_2d(&acc, &c, Bop::Intersection);
            }
            acc
        }
        Node::Hull(children) => {
            let pts: Vec<Point2> = children.iter().flat_map(render2d).flatten().collect();
            hull_2d(pts)
        }
        Node::Minkowski(children) => {
            let sets: Vec<Vec<Contour>> = children
                .iter()
                .map(render2d)
                .filter(|s| !s.is_empty())
                .collect();
            minkowski_2d(sets)
        }
        // Display attributes and provenance are transparent to 2D geometry; `%`
        // background is excluded from the fused/exported profile.
        Node::Color { child, .. } | Node::Highlight(child) | Node::Provenance { child, .. } => {
            render2d(child)
        }
        Node::Background(_) => Vec::new(),
        // A projection anywhere in a 2D subtree flattens its 3D child; rendered
        // via the geometry layer, not here (needs a mesh). Handled by render_node.
        _ => Vec::new(),
    }
}

/// Exact 2D Minkowski sum of several contour sets. Minkowski distributes over
/// union, and a triangle⊕triangle is convex (the hull of the 9 vertex sums is
/// exact), so decompose each operand into triangles (earcut), sum every pair,
/// and union the pieces — correct for non-convex operands (e.g. rounding an
/// L-outline or gear), not just the convex hull.
fn minkowski_2d(sets: Vec<Vec<Contour>>) -> Vec<Contour> {
    let mut it = sets.into_iter().filter(|s| !s.is_empty());
    let Some(mut acc) = it.next() else {
        return Vec::new();
    };
    for s in it {
        acc = minkowski_pair_2d(&acc, &s);
        if acc.is_empty() {
            break;
        }
    }
    acc
}

/// The convex triangles of a contour set (earcut, holes cut out).
fn triangles_2d(contours: &[Contour]) -> Vec<[Point2; 3]> {
    let (points, _ranges, tris) = prepare(contours);
    tris.iter()
        .map(|t| {
            [
                points[t[0] as usize],
                points[t[1] as usize],
                points[t[2] as usize],
            ]
        })
        .collect()
}

fn minkowski_pair_2d(a: &[Contour], b: &[Contour]) -> Vec<Contour> {
    let (ta, tb) = (triangles_2d(a), triangles_2d(b));
    if ta.is_empty() || tb.is_empty() {
        return Vec::new();
    }
    let mut pieces: Vec<MultiPolygon<f64>> = Vec::new();
    for x in &ta {
        for y in &tb {
            let mut sums = Vec::with_capacity(9);
            for p in x {
                for q in y {
                    sums.push([p[0] + q[0], p[1] + q[1]]);
                }
            }
            for c in hull_2d(sums) {
                if c.len() >= 3 {
                    pieces.push(MultiPolygon::new(vec![Polygon::new(
                        to_linestring(&c),
                        vec![],
                    )]));
                }
            }
        }
    }
    union_multi(pieces)
        .map(from_multipolygon)
        .unwrap_or_default()
}

fn map_contours(cs: Vec<Contour>, f: impl Fn(Point2) -> Point2) -> Vec<Contour> {
    cs.into_iter()
        .map(|c| c.into_iter().map(&f).collect())
        .collect()
}

/// 2D `resize`: scale the contours so their bounding box matches `new` (0 = keep;
/// an `auto` axis with no target adopts the first explicit factor).
fn resize2d(contours: Vec<Contour>, new: [f64; 3], auto: [bool; 3]) -> Vec<Contour> {
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for c in &contours {
        for p in c {
            for i in 0..2 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
    }
    if lo[0] > hi[0] {
        return contours;
    }
    let size = [hi[0] - lo[0], hi[1] - lo[1]];
    let mut factor = [1.0; 2];
    let mut explicit = None;
    for i in 0..2 {
        if new[i] > 0.0 && size[i] > 0.0 {
            factor[i] = new[i] / size[i];
            explicit.get_or_insert(factor[i]);
        }
    }
    if let Some(f) = explicit {
        for i in 0..2 {
            if new[i] == 0.0 && auto[i] {
                factor[i] = f;
            }
        }
    }
    map_contours(contours, |p| [p[0] * factor[0], p[1] * factor[1]])
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
pub(crate) fn chain_segments(segs: Vec<(Point2, Point2)>) -> Vec<Contour> {
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
            match inc
                .get(&key(cur))
                .and_then(|v| v.iter().find(|&&j| !used[j]).copied())
            {
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
/// chamfers). Positive grows, negative shrinks.
///
/// The offset region is assembled from **convex pieces** (an offset slab per
/// edge plus a join cap per corner) unioned/subtracted through the `geo`
/// clipper, rather than a single per-vertex ring — a folding ring
/// self-intersects on concave insets and would fill the bowtie. Growing is
/// `solid ∪ band`; shrinking is `solid − band`, so an inset larger than a local
/// feature collapses to empty (matching OpenSCAD) instead of a wrong solid.
pub fn offset(
    contours: &[Contour],
    r: f64,
    delta: f64,
    chamfer: bool,
    frags: FragmentSpec,
) -> Vec<Contour> {
    let (amt, rounded) = if r != 0.0 { (r, true) } else { (delta, false) };
    let solid = to_multipolygon(contours);
    if amt == 0.0 {
        return from_multipolygon(solid);
    }
    // Orient each boundary (outer CCW, holes CW) so the edge normal points out
    // of the solid; the band then grows/shrinks holes correctly too.
    let mut pieces: Vec<Contour> = Vec::new();
    for (outer, holes) in group_contours(contours) {
        offset_pieces(&outer, amt, rounded, chamfer, frags, &mut pieces);
        for h in &holes {
            offset_pieces(h, amt, rounded, chamfer, frags, &mut pieces);
        }
    }
    let band = clean_union(&pieces);
    let result = if amt > 0.0 {
        solid.union(&band)
    } else {
        solid.difference(&band)
    };
    from_multipolygon(result)
}

/// Union a set of convex pieces (each taken as a filled CCW region) through the
/// clipper into a clean multipolygon. The pieces are convex, so no
/// self-intersection can arise.
fn clean_union(pieces: &[Contour]) -> MultiPolygon<f64> {
    let mps: Vec<MultiPolygon<f64>> = pieces
        .iter()
        .filter(|c| c.len() >= 3 && signed_area(c).abs() > 1e-12)
        .map(|c| {
            let ccw: Contour = if signed_area(c) < 0.0 {
                c.iter().rev().cloned().collect()
            } else {
                c.clone()
            };
            MultiPolygon::new(vec![Polygon::new(to_linestring(&ccw), vec![])])
        })
        .collect();
    union_multi(mps).unwrap_or_else(|| MultiPolygon::new(Vec::new()))
}

/// Emit the convex offset pieces for one oriented boundary contour into `out`:
/// an offset slab quad per edge, plus a join cap (round arc / miter / chamfer)
/// at each corner whose outer side gaps open. `poly` must be oriented so that
/// its right-hand edge normal points out of the solid (outer CCW, holes CW);
/// `amt` is signed (outward slabs when growing, inward when shrinking).
fn offset_pieces(
    poly: &[Point2],
    amt: f64,
    rounded: bool,
    chamfer: bool,
    frags: FragmentSpec,
    out: &mut Vec<Contour>,
) {
    let n = poly.len();
    if n < 3 {
        return;
    }
    let seg_full = fragments(amt.abs(), frags).max(3) as f64;

    let edge_normal = |i: usize| -> Point2 {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let d = [b[0] - a[0], b[1] - a[1]];
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-12);
        [d[1] / len, -d[0] / len]
    };

    // One slab per edge: the edge and its offset copy.
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let nrm = edge_normal(i);
        let ao = [a[0] + amt * nrm[0], a[1] + amt * nrm[1]];
        let bo = [b[0] + amt * nrm[0], b[1] + amt * nrm[1]];
        out.push(vec![a, b, bo, ao]);
    }

    // One join cap per corner that gaps open on its outer side (convex when
    // growing, reflex when shrinking) — reflex/convex on the other side is
    // already covered by the overlapping slabs.
    for i in 0..n {
        let vi = poly[i];
        let n_in = edge_normal((i + n - 1) % n);
        let n_out = edge_normal(i);
        let p_in = [vi[0] + amt * n_in[0], vi[1] + amt * n_in[1]];
        let p_out = [vi[0] + amt * n_out[0], vi[1] + amt * n_out[1]];

        let din = [
            vi[0] - poly[(i + n - 1) % n][0],
            vi[1] - poly[(i + n - 1) % n][1],
        ];
        let dout = [poly[(i + 1) % n][0] - vi[0], poly[(i + 1) % n][1] - vi[1]];
        let convex = din[0] * dout[1] - din[1] * dout[0] > 0.0;
        let fill = (amt > 0.0 && convex) || (amt < 0.0 && !convex);
        if !fill {
            continue;
        }

        if rounded {
            let a0 = n_in[1].atan2(n_in[0]);
            let a1 = n_out[1].atan2(n_out[0]);
            let mut da = a1 - a0;
            while da <= -PI {
                da += 2.0 * PI;
            }
            while da > PI {
                da -= 2.0 * PI;
            }
            let steps = ((seg_full * (da.abs() / (2.0 * PI))).ceil() as usize).max(1);
            let mut cap = vec![vi];
            for s in 0..=steps {
                let a = a0 + da * (s as f64 / steps as f64);
                cap.push([vi[0] + amt * a.cos(), vi[1] + amt * a.sin()]);
            }
            out.push(cap);
        } else if chamfer {
            out.push(vec![vi, p_in, p_out]);
        } else {
            // miter: apex is where the two offset edge-lines meet.
            match line_intersect(p_in, din, p_out, dout) {
                Some(m) => out.push(vec![vi, p_in, m, p_out]),
                None => out.push(vec![vi, p_in, p_out]),
            }
        }
    }
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

/// Is `pt` inside the simple polygon `poly` (ray-cast, even-odd)?
fn point_in_polygon(poly: &[Point2], pt: Point2) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (pi, pj) = (poly[i], poly[j]);
        if ((pi[1] > pt[1]) != (pj[1] > pt[1]))
            && (pt[0] < (pj[0] - pi[0]) * (pt[1] - pi[1]) / (pj[1] - pi[1]) + pi[0])
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The concatenated vertex list, each contour's `(start, len)` range in it, and
/// the cap triangulation (indices into the vertex list). See [`prepare`].
type PreparedContours = (Vec<Point2>, Vec<(usize, usize)>, Vec<[u32; 3]>);

/// Remove consecutive duplicate points (within the kernel's weld tolerance),
/// including a final point that repeats the first, so the contour has no
/// zero-length edges.
fn dedup_consecutive(c: &Contour) -> Contour {
    const EPS: f64 = 1e-7;
    let close = |a: &Point2, b: &Point2| (a[0] - b[0]).abs() <= EPS && (a[1] - b[1]).abs() <= EPS;
    let mut out: Contour = Vec::with_capacity(c.len());
    for p in c {
        if out.last().is_none_or(|q| !close(q, p)) {
            out.push(*p);
        }
    }
    if out.len() >= 2 && close(&out[0], out.last().unwrap()) {
        out.pop();
    }
    out
}

/// Prepare a set of contours (with even-odd nesting → outers + holes) for
/// filling and extrusion. Returns the concatenated vertex list, each contour's
/// `(start, len)` range in it (outers oriented CCW, holes CW), and the cap
/// triangulation (indices into the vertex list), with holes cut out via earcut.
fn prepare(contours: &[Contour]) -> PreparedContours {
    // Drop consecutive duplicate vertices (and any closing repeat of the first)
    // from each contour. A zero-length edge would otherwise extrude into a
    // degenerate side wall — a quad with two coincident corners — which the
    // manifold kernel rejects as non-manifold. Generated profiles routinely
    // emit such duplicates (e.g. BOSL2's `rack2d`).
    let cleaned: Vec<Contour> = contours.iter().map(|c| dedup_consecutive(c)).collect();
    let valid: Vec<&Contour> = cleaned.iter().filter(|c| c.len() >= 3).collect();
    let n = valid.len();
    if n == 0 {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    // Nesting depth of each contour (how many others contain a point of it).
    let rep: Vec<Point2> = valid.iter().map(|c| c[0]).collect();
    let depth: Vec<usize> = (0..n)
        .map(|i| {
            (0..n)
                .filter(|&j| j != i && point_in_polygon(valid[j], rep[i]))
                .count()
        })
        .collect();

    // Orient: outers (even depth) CCW, holes (odd depth) CW.
    let oriented: Vec<Contour> = valid
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let want_ccw = depth[i] % 2 == 0;
            if (signed_area(c) > 0.0) == want_ccw {
                (*c).clone()
            } else {
                c.iter().rev().cloned().collect()
            }
        })
        .collect();

    let mut points: Vec<Point2> = Vec::new();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for c in &oriented {
        ranges.push((points.len(), c.len()));
        points.extend_from_slice(c);
    }

    // Cap triangulation: earcut each outer with its immediate holes.
    let mut cap_tris: Vec<[u32; 3]> = Vec::new();
    for i in 0..n {
        if depth[i] % 2 != 0 {
            continue; // hole; handled by its parent outer
        }
        let holes: Vec<usize> = (0..n)
            .filter(|&h| depth[h] == depth[i] + 1 && point_in_polygon(&oriented[i], rep[h]))
            .collect();

        let mut flat: Vec<f64> = Vec::new();
        let mut map: Vec<u32> = Vec::new(); // group vertex index -> global index
        let mut hole_starts: Vec<usize> = Vec::new();
        let push_ring = |flat: &mut Vec<f64>, map: &mut Vec<u32>, idx: usize| {
            let (s, len) = ranges[idx];
            for k in 0..len {
                let g = (s + k) as u32;
                flat.push(points[g as usize][0]);
                flat.push(points[g as usize][1]);
                map.push(g);
            }
        };
        push_ring(&mut flat, &mut map, i);
        for &h in &holes {
            hole_starts.push(map.len());
            push_ring(&mut flat, &mut map, h);
        }
        if let Ok(idx) = earcutr::earcut(&flat, &hole_starts, 2) {
            for t in idx.chunks(3) {
                if t.len() == 3 {
                    cap_tris.push([map[t[0]], map[t[1]], map[t[2]]]);
                }
            }
        }
    }
    (points, ranges, cap_tris)
}

/// A flat mesh of a 2D shape at z=0 (used when a 2D node is the render target),
/// with holes cut out (even-odd).
pub fn flat_mesh(contours: &[Contour]) -> Mesh {
    let (points, _ranges, cap_tris) = prepare(contours);
    let mut mesh = Mesh::new();
    mesh.verts = points.iter().map(|p| [p[0], p[1], 0.0]).collect();
    mesh.tris = cap_tris;
    mesh
}

/// `linear_extrude` of the contours to a mesh, cutting out holes (even-odd) in
/// the caps and giving every contour (outer and hole) a wall loop.
pub fn linear_extrude(
    contours: &[Contour],
    height: f64,
    center: bool,
    twist: f64,
    scale: Point2,
    slices: u32,
) -> Mesh {
    let (points, ranges, cap_tris) = prepare(contours);
    let mut mesh = Mesh::new();
    if points.is_empty() {
        return mesh;
    }
    let n = points.len();
    let slices = slices.max(1);
    let z0 = if center { -height / 2.0 } else { 0.0 };

    // `slices+1` rings of all points, twisted/scaled per layer.
    for layer in 0..=slices {
        let t = layer as f64 / slices as f64;
        let ang = (-twist * t).to_radians();
        let (s, c) = (ang.sin(), ang.cos());
        let sx = 1.0 + (scale[0] - 1.0) * t;
        let sy = 1.0 + (scale[1] - 1.0) * t;
        let z = z0 + height * t;
        for p in &points {
            let (x, y) = (p[0] * sx, p[1] * sy);
            mesh.verts.push([x * c - y * s, x * s + y * c, z]);
        }
    }
    let ring = |layer: u32, i: usize| layer * n as u32 + i as u32;

    // Walls: each contour range forms a loop at every layer.
    for &(start, len) in &ranges {
        for layer in 0..slices {
            for k in 0..len {
                let i = start + k;
                let j = start + (k + 1) % len;
                let (a, b) = (ring(layer, i), ring(layer, j));
                let (cc, d) = (ring(layer + 1, j), ring(layer + 1, i));
                mesh.tris.push([a, b, cc]);
                mesh.tris.push([a, cc, d]);
            }
        }
    }

    // Caps: bottom (reversed) + top, holes already removed by earcut.
    for t in &cap_tris {
        let (a, b, cc) = (t[0] as usize, t[1] as usize, t[2] as usize);
        mesh.tris.push([ring(0, a), ring(0, cc), ring(0, b)]);
        mesh.tris
            .push([ring(slices, a), ring(slices, b), ring(slices, cc)]);
    }

    mesh.ensure_outward();
    mesh
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
        let frac = k as f64 / steps as f64;
        let th = (angle * frac).to_radians();
        let (s, c) = (th.sin(), th.cos());
        for p in contour {
            // 2D point (x=radius, y=height) -> 3D ring.
            mesh.verts.push([p[0] * c, p[0] * s, p[1]]);
        }
    }
    let ring = |k: u32, i: usize| base + (k % ring_count) * n as u32 + i as u32;
    // Walls span `steps` sectors whether or not the sweep is a full revolution
    // (the extra open-arc ring is a cap boundary, not another wall sector).
    let wall_steps = steps;
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
            mesh.tris
                .push([ring(0, tri[0]), ring(0, tri[2]), ring(0, tri[1])]);
            mesh.tris.push([
                ring(steps, tri[0]),
                ring(steps, tri[1]),
                ring(steps, tri[2]),
            ]);
        }
    }
}

#[cfg(test)]
mod offset_tests {
    use super::*;

    fn square(s: f64) -> Contour {
        vec![[0.0, 0.0], [s, 0.0], [s, s], [0.0, s]]
    }
    /// Net filled area (outers positive, holes negative).
    fn area(cs: &[Contour]) -> f64 {
        cs.iter().map(|c| signed_area(c)).sum::<f64>().abs()
    }

    #[test]
    fn offset_grow_miter_square() {
        let r = offset(&[square(10.0)], 0.0, 2.0, false, FragmentSpec::default());
        assert!((area(&r) - 196.0).abs() < 1e-6, "grow area {}", area(&r));
    }

    #[test]
    fn offset_mild_inset_square() {
        let r = offset(&[square(10.0)], 0.0, -2.0, false, FragmentSpec::default());
        assert!((area(&r) - 36.0).abs() < 1e-6, "inset area {}", area(&r));
    }

    /// An inset larger than the shape must collapse to nothing, not a
    /// self-intersecting bowtie (the A3 bug).
    #[test]
    fn offset_over_inset_collapses_to_empty() {
        let r = offset(&[square(10.0)], 0.0, -10.0, false, FragmentSpec::default());
        assert!(r.is_empty(), "over-inset should be empty, got {r:?}");
    }

    /// Convex ⊕ convex stays exact: [0,10]² ⊕ [0,2]² = [0,12]² (area 144).
    #[test]
    fn minkowski_2d_convex_is_exact() {
        let r = minkowski_2d(vec![vec![square(10.0)], vec![square(2.0)]]);
        assert!(
            (area(&r) - 144.0).abs() < 1e-6,
            "square⊕square area {}",
            area(&r)
        );
    }

    /// A non-convex operand keeps its concavity: the L ⊕ square area is strictly
    /// below the convex-hull approximation (the A5 bug) and above the un-grown L.
    #[test]
    fn minkowski_2d_nonconvex_beats_convex_hull() {
        let l: Contour = vec![
            [0.0, 0.0],
            [24.0, 0.0],
            [24.0, 6.0],
            [6.0, 6.0],
            [6.0, 24.0],
            [0.0, 24.0],
        ];
        let got = area(&minkowski_2d(vec![vec![l.clone()], vec![square(2.0)]]));
        // Old convex approximation: hull of all pairwise vertex sums.
        let mut sums = Vec::new();
        for a in &l {
            for b in &square(2.0) {
                sums.push([a[0] + b[0], a[1] + b[1]]);
            }
        }
        let convex_area = area(&hull_2d(sums));
        assert!(got > 252.0, "should exceed the L's own area (252): {got}");
        assert!(
            got < convex_area - 10.0,
            "exact {got} not below convex approx {convex_area}"
        );
    }
}
