//! Geometry: mesh types, the fragment formula + primitive tessellation, the
//! `Kernel` trait (CSG boolean backend), and the CSG-tree -> mesh renderer.

mod hull;
mod kernel;
mod mesh;
mod shape2d;
mod tessellate;

pub use kernel::{BoolmeshKernel, Kernel};
#[cfg(not(target_arch = "wasm32"))]
pub use kernel::ManifoldKernel;
pub use mesh::Mesh;
pub use tessellate::{cube, cylinder, fragments, polyhedron, sphere};

use quito_ir::{Node, Vec3};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, thiserror::Error)]
pub enum GeomError {
    #[error("kernel error: {0}")]
    Kernel(String),
    #[error("input geometry is not manifold: {0}")]
    NonManifold(String),
}

/// A content-addressed geometry cache (M4): maps a structural hash of a CSG
/// subtree to its rendered mesh. Reused across renders, it makes warm edits
/// incremental — only subtrees whose structure changed are re-rendered, the
/// rest are cheap `Mesh` clones; within a single render it also deduplicates
/// identical subtrees (common-subexpression elimination).
#[derive(Default)]
pub struct GeomCache {
    meshes: HashMap<u64, Mesh>,
}

impl GeomCache {
    pub fn new() -> Self {
        GeomCache::default()
    }
    /// Number of cached subtrees.
    pub fn len(&self) -> usize {
        self.meshes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.meshes.is_empty()
    }
    /// Drop all cached meshes.
    pub fn clear(&mut self) {
        self.meshes.clear();
    }
}

/// Shared state threaded through a single render traversal.
struct Ctx<'a> {
    kernel: &'a dyn Kernel,
    cache: &'a mut GeomCache,
    /// Precomputed structural hash of every node in the tree, by address.
    hashes: &'a HashMap<*const Node, u64>,
}

/// Render a CSG tree to a mesh using the default kernel for the target:
/// C++ Manifold on native, pure-Rust boolmesh on wasm.
#[cfg(not(target_arch = "wasm32"))]
pub fn render(node: &Node) -> Result<Mesh, GeomError> {
    render_with(node, &ManifoldKernel::new())
}

/// Render a CSG tree to a mesh using the default kernel for the target:
/// C++ Manifold on native, pure-Rust boolmesh on wasm.
#[cfg(target_arch = "wasm32")]
pub fn render(node: &Node) -> Result<Mesh, GeomError> {
    render_with(node, &BoolmeshKernel::new())
}

/// Render a CSG tree to a mesh using the given kernel (no persistent cache).
pub fn render_with(node: &Node, kernel: &dyn Kernel) -> Result<Mesh, GeomError> {
    let mut cache = GeomCache::new();
    render_cached(node, kernel, &mut cache)
}

/// Render using the given kernel and a caller-owned [`GeomCache`], enabling
/// incremental warm-edit re-renders (unchanged subtrees are not recomputed).
pub fn render_cached(
    node: &Node,
    kernel: &dyn Kernel,
    cache: &mut GeomCache,
) -> Result<Mesh, GeomError> {
    let mut hashes = HashMap::new();
    hash_all(node, &mut hashes);
    let mut ctx = Ctx { kernel, cache, hashes: &hashes };
    render_node(node, &mut ctx)
}

/// Memoized render of one node: cache hit → clone; miss → render + store.
fn render_node(node: &Node, ctx: &mut Ctx) -> Result<Mesh, GeomError> {
    let key = ctx.hashes[&(node as *const Node)];
    if let Some(m) = ctx.cache.meshes.get(&key) {
        return Ok(m.clone());
    }
    let mesh = render_uncached(node, ctx)?;
    ctx.cache.meshes.insert(key, mesh.clone());
    Ok(mesh)
}

/// The actual per-variant renderer (children go back through [`render_node`]).
/// Is `node` a 2D subtree (result lies in the XY plane)?
fn is_2d(node: &Node) -> bool {
    use Node::*;
    match node {
        Square { .. } | Circle { .. } | Polygon { .. } | Offset { .. } | Projection { .. } => true,
        Cube { .. } | Sphere { .. } | Cylinder { .. } | Polyhedron { .. } | Import { .. }
        | LinearExtrude { .. } | RotateExtrude { .. } | Empty => false,
        Translate { child, .. }
        | Rotate { child, .. }
        | Scale { child, .. }
        | Mirror { child, .. }
        | MultMatrix { child, .. }
        | Resize { child, .. } => is_2d(child),
        Group(cs) | Union(cs) | Difference(cs) | Intersection(cs) | Hull(cs) | Minkowski(cs) => {
            cs.iter().any(is_2d)
        }
    }
}

fn render_uncached(node: &Node, ctx: &mut Ctx) -> Result<Mesh, GeomError> {
    // 2D CSG (boolean/hull/minkowski/group of 2D shapes) is clipped in the 2D
    // plane and returned as a flat mesh — the 3D kernel can't handle the
    // coplanar, zero-volume meshes these would otherwise produce.
    if matches!(
        node,
        Node::Group(_)
            | Node::Union(_)
            | Node::Difference(_)
            | Node::Intersection(_)
            | Node::Hull(_)
            | Node::Minkowski(_)
    ) && is_2d(node)
    {
        return Ok(shape2d::flat_mesh(&shape2d::render2d(node)));
    }
    match node {
        Node::Empty => Ok(Mesh::new()),
        Node::Cube { size, center } => Ok(cube(*size, *center)),
        Node::Sphere { r, frags } => Ok(sphere(*r, *frags)),
        Node::Cylinder {
            h,
            r1,
            r2,
            center,
            frags,
        } => Ok(cylinder(*h, *r1, *r2, *center, *frags)),
        Node::Polyhedron { points, faces } => Ok(polyhedron(points, faces)),
        Node::Import { data, format } => Ok(match format.as_str() {
            "off" => Mesh::from_off(&String::from_utf8_lossy(data)),
            "obj" => Mesh::from_obj(&String::from_utf8_lossy(data)),
            _ => Mesh::from_stl(data), // stl (binary or ascii)
        }),

        // 2D shapes rendered as a flat mesh at z=0.
        Node::Square { .. } | Node::Circle { .. } | Node::Polygon { .. } | Node::Offset { .. } => {
            Ok(shape2d::flat_mesh(&shape2d::render2d(node)))
        }
        Node::LinearExtrude {
            height,
            center,
            twist,
            scale,
            slices,
            child,
        } => extrude_csg(
            child,
            &|cs| shape2d::linear_extrude(cs, *height, *center, *twist, *scale, *slices),
            ctx,
        ),
        Node::RotateExtrude { angle, frags, child } => {
            extrude_csg(child, &|cs| shape2d::rotate_extrude(cs, *angle, *frags), ctx)
        }
        Node::Projection { cut, child } => {
            let mesh = render_node(child, ctx)?;
            let contours = if *cut {
                shape2d::slice_z0(&mesh)
            } else {
                shape2d::silhouette(&mesh)
            };
            Ok(shape2d::flat_mesh(&contours))
        }

        Node::Group(children) => {
            let meshes = render_all(children, ctx)?;
            ctx.kernel.union(meshes)
        }
        Node::Union(children) => {
            let meshes = render_all(children, ctx)?;
            ctx.kernel.union(meshes)
        }
        Node::Intersection(children) => {
            let meshes = render_all(children, ctx)?;
            ctx.kernel.intersection(meshes)
        }
        Node::Hull(children) => {
            let meshes = render_all(children, ctx)?;
            ctx.kernel.hull(meshes)
        }
        Node::Minkowski(children) => {
            let meshes = render_all(children, ctx)?;
            Ok(minkowski_fold(meshes))
        }
        Node::Difference(children) => {
            let mut meshes = render_all(children, ctx)?;
            if meshes.is_empty() {
                Ok(Mesh::new())
            } else {
                let base = meshes.remove(0);
                ctx.kernel.difference(base, meshes)
            }
        }

        Node::Translate { v, child } => {
            let mut m = render_node(child, ctx)?;
            translate(&mut m, *v);
            Ok(m)
        }
        Node::Rotate { deg, child } => {
            let mut m = render_node(child, ctx)?;
            rotate(&mut m, *deg);
            Ok(m)
        }
        Node::Scale { v, child } => {
            let mut m = render_node(child, ctx)?;
            scale(&mut m, *v);
            Ok(m)
        }
        Node::Mirror { v, child } => {
            let mut m = render_node(child, ctx)?;
            mirror(&mut m, *v);
            Ok(m)
        }
        Node::MultMatrix { m: mat, child } => {
            let mut mesh = render_node(child, ctx)?;
            mult_matrix(&mut mesh, mat);
            Ok(mesh)
        }
        Node::Resize { new, auto, child } => {
            let mut mesh = render_node(child, ctx)?;
            resize(&mut mesh, *new, *auto);
            Ok(mesh)
        }
    }
}

fn render_all(children: &[Node], ctx: &mut Ctx) -> Result<Vec<Mesh>, GeomError> {
    children.iter().map(|c| render_node(c, ctx)).collect()
}

/// Structural hash of every node in the tree, keyed by node address. Computed
/// once per render in a single O(n) post-order pass (child hashes combine into
/// the parent's), so [`render_node`] can look up any subtree's hash in O(1)
/// without re-traversing it.
fn hash_all(node: &Node, out: &mut HashMap<*const Node, u64>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::mem::discriminant(node).hash(&mut h);
    let bits = |x: &f64, h: &mut std::collections::hash_map::DefaultHasher| x.to_bits().hash(h);
    let frags = |f: &quito_ir::FragmentSpec, h: &mut std::collections::hash_map::DefaultHasher| {
        f.fn_.to_bits().hash(h);
        f.fa.to_bits().hash(h);
        f.fs.to_bits().hash(h);
    };
    match node {
        Node::Empty => {}
        Node::Group(cs)
        | Node::Union(cs)
        | Node::Difference(cs)
        | Node::Intersection(cs)
        | Node::Hull(cs)
        | Node::Minkowski(cs) => {
            for c in cs {
                hash_all(c, out).hash(&mut h);
            }
        }
        Node::Cube { size, center } => {
            for x in size {
                bits(x, &mut h);
            }
            center.hash(&mut h);
        }
        Node::Sphere { r, frags: f } => {
            bits(r, &mut h);
            frags(f, &mut h);
        }
        Node::Cylinder { h: hh, r1, r2, center, frags: f } => {
            bits(hh, &mut h);
            bits(r1, &mut h);
            bits(r2, &mut h);
            center.hash(&mut h);
            frags(f, &mut h);
        }
        Node::Polyhedron { points, faces } => {
            for p in points {
                for x in p {
                    bits(x, &mut h);
                }
            }
            faces.hash(&mut h);
        }
        Node::Square { size, center } => {
            for x in size {
                bits(x, &mut h);
            }
            center.hash(&mut h);
        }
        Node::Circle { r, frags: f } => {
            bits(r, &mut h);
            frags(f, &mut h);
        }
        Node::Polygon { points, paths } => {
            for p in points {
                for x in p {
                    bits(x, &mut h);
                }
            }
            paths.hash(&mut h);
        }
        Node::LinearExtrude { height, center, twist, scale, slices, child } => {
            bits(height, &mut h);
            center.hash(&mut h);
            bits(twist, &mut h);
            for x in scale {
                bits(x, &mut h);
            }
            slices.hash(&mut h);
            hash_all(child, out).hash(&mut h);
        }
        Node::RotateExtrude { angle, frags: f, child } => {
            bits(angle, &mut h);
            frags(f, &mut h);
            hash_all(child, out).hash(&mut h);
        }
        Node::Offset { r, delta, chamfer, frags: f, child } => {
            bits(r, &mut h);
            bits(delta, &mut h);
            chamfer.hash(&mut h);
            frags(f, &mut h);
            hash_all(child, out).hash(&mut h);
        }
        Node::Translate { v, child } | Node::Scale { v, child } | Node::Mirror { v, child } => {
            for x in v {
                bits(x, &mut h);
            }
            hash_all(child, out).hash(&mut h);
        }
        Node::Rotate { deg, child } => {
            for x in deg {
                bits(x, &mut h);
            }
            hash_all(child, out).hash(&mut h);
        }
        Node::MultMatrix { m, child } => {
            for row in m {
                for x in row {
                    bits(x, &mut h);
                }
            }
            hash_all(child, out).hash(&mut h);
        }
        Node::Resize { new, auto, child } => {
            for x in new {
                bits(x, &mut h);
            }
            auto.hash(&mut h);
            hash_all(child, out).hash(&mut h);
        }
        Node::Import { data, format } => {
            data.hash(&mut h);
            format.hash(&mut h);
        }
        Node::Projection { cut, child } => {
            cut.hash(&mut h);
            hash_all(child, out).hash(&mut h);
        }
    }
    let val = h.finish();
    out.insert(node as *const Node, val);
    val
}

/// Minkowski sum of a chain of meshes. Exact for convex operands (the common
/// rounding case, e.g. `minkowski(){ cube; sphere; }`); for non-convex operands
/// it is the convex Minkowski approximation. After the first sum the accumulator
/// is convex, so the rest are exact.
fn minkowski_fold(meshes: Vec<Mesh>) -> Mesh {
    let mut it = meshes.into_iter().filter(|m| !m.is_empty());
    let Some(mut acc) = it.next() else {
        return Mesh::new();
    };
    for m in it {
        acc = minkowski_pair(&acc, &m);
    }
    acc
}

fn minkowski_pair(a: &Mesh, b: &Mesh) -> Mesh {
    let mut pts = Vec::with_capacity(a.verts.len() * b.verts.len());
    for va in &a.verts {
        for vb in &b.verts {
            pts.push([va[0] + vb[0], va[1] + vb[1], va[2] + vb[2]]);
        }
    }
    hull::convex_hull(&pts)
}

/// Extrude a 2D subtree, distributing over 2D booleans: because the extrusion
/// transform is applied identically to every operand (per height slice / per
/// revolution step), `extrude(A op B) == extrude(A) op extrude(B)`, so 2D CSG
/// is realized with the existing 3D kernel (no separate 2D kernel needed).
fn extrude_csg(
    node: &Node,
    extrude: &dyn Fn(&[shape2d::Contour]) -> Mesh,
    ctx: &mut Ctx,
) -> Result<Mesh, GeomError> {
    match node {
        Node::Empty => Ok(Mesh::new()),
        Node::Union(children) | Node::Group(children) => {
            let meshes = children
                .iter()
                .map(|c| extrude_csg(c, extrude, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            ctx.kernel.union(meshes)
        }
        Node::Intersection(children) => {
            let meshes = children
                .iter()
                .map(|c| extrude_csg(c, extrude, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            ctx.kernel.intersection(meshes)
        }
        Node::Difference(children) => {
            let mut meshes = children
                .iter()
                .map(|c| extrude_csg(c, extrude, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            if meshes.is_empty() {
                Ok(Mesh::new())
            } else {
                let base = meshes.remove(0);
                ctx.kernel.difference(base, meshes)
            }
        }
        // projection: render the 3D child, flatten, then extrude.
        Node::Projection { cut, child } => {
            let mesh = render_node(child, ctx)?;
            let contours = if *cut {
                shape2d::slice_z0(&mesh)
            } else {
                shape2d::silhouette(&mesh)
            };
            Ok(extrude(&contours))
        }
        // A leaf 2D shape (primitive or transform chain): render to contours.
        leaf => Ok(extrude(&shape2d::render2d(leaf))),
    }
}

fn translate(m: &mut Mesh, v: Vec3) {
    for p in &mut m.verts {
        p[0] += v[0];
        p[1] += v[1];
        p[2] += v[2];
    }
}

fn scale(m: &mut Mesh, v: Vec3) {
    for p in &mut m.verts {
        p[0] *= v[0];
        p[1] *= v[1];
        p[2] *= v[2];
    }
    // A negative determinant mirrors the mesh, inverting winding.
    if v[0] * v[1] * v[2] < 0.0 {
        m.flip_winding();
    }
}

/// Reflect across the plane through the origin with normal `v`.
fn mirror(m: &mut Mesh, v: Vec3) {
    let d = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if d == 0.0 {
        return;
    }
    // Householder reflection I - 2 v vᵀ / (v·v).
    let h = [
        [1.0 - 2.0 * v[0] * v[0] / d, -2.0 * v[0] * v[1] / d, -2.0 * v[0] * v[2] / d],
        [-2.0 * v[1] * v[0] / d, 1.0 - 2.0 * v[1] * v[1] / d, -2.0 * v[1] * v[2] / d],
        [-2.0 * v[2] * v[0] / d, -2.0 * v[2] * v[1] / d, 1.0 - 2.0 * v[2] * v[2] / d],
    ];
    for p in &mut m.verts {
        let [x, y, z] = *p;
        *p = [
            h[0][0] * x + h[0][1] * y + h[0][2] * z,
            h[1][0] * x + h[1][1] * y + h[1][2] * z,
            h[2][0] * x + h[2][1] * y + h[2][2] * z,
        ];
    }
    m.flip_winding(); // reflection inverts orientation
}

/// Apply a 4x4 affine matrix (row-major).
fn mult_matrix(m: &mut Mesh, mat: &[[f64; 4]; 4]) {
    for p in &mut m.verts {
        let [x, y, z] = *p;
        *p = [
            mat[0][0] * x + mat[0][1] * y + mat[0][2] * z + mat[0][3],
            mat[1][0] * x + mat[1][1] * y + mat[1][2] * z + mat[1][3],
            mat[2][0] * x + mat[2][1] * y + mat[2][2] * z + mat[2][3],
        ];
    }
    // Flip winding if the linear part has negative determinant.
    let det = mat[0][0] * (mat[1][1] * mat[2][2] - mat[1][2] * mat[2][1])
        - mat[0][1] * (mat[1][0] * mat[2][2] - mat[1][2] * mat[2][0])
        + mat[0][2] * (mat[1][0] * mat[2][1] - mat[1][1] * mat[2][0]);
    if det < 0.0 {
        m.flip_winding();
    }
}

/// Scale so the bounding box matches `new` (0 = keep; `auto` scales that axis
/// by another axis's factor).
fn resize(m: &mut Mesh, new: Vec3, auto: [bool; 3]) {
    let Some((lo, hi)) = m.bbox() else { return };
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let mut factor = [1.0; 3];
    let mut explicit = None;
    for i in 0..3 {
        if new[i] > 0.0 && size[i] > 0.0 {
            factor[i] = new[i] / size[i];
            if explicit.is_none() {
                explicit = Some(factor[i]);
            }
        }
    }
    // auto axes with no explicit target adopt the first explicit factor.
    if let Some(f) = explicit {
        for i in 0..3 {
            if new[i] == 0.0 && auto[i] {
                factor[i] = f;
            }
        }
    }
    for p in &mut m.verts {
        for i in 0..3 {
            p[i] *= factor[i];
        }
    }
    if factor[0] * factor[1] * factor[2] < 0.0 {
        m.flip_winding();
    }
}

/// Rotate by Euler angles (degrees), applied X then Y then Z (OpenSCAD order:
/// the combined matrix is Rz * Ry * Rx).
fn rotate(m: &mut Mesh, deg: Vec3) {
    let (a, b, c) = (
        deg[0].to_radians(),
        deg[1].to_radians(),
        deg[2].to_radians(),
    );
    let (sa, ca) = (a.sin(), a.cos());
    let (sb, cb) = (b.sin(), b.cos());
    let (sc, cc) = (c.sin(), c.cos());
    for p in &mut m.verts {
        let [x, y, z] = *p;
        // Rx
        let (y1, z1) = (y * ca - z * sa, y * sa + z * ca);
        let x1 = x;
        // Ry
        let (x2, z2) = (x1 * cb + z1 * sb, -x1 * sb + z1 * cb);
        let y2 = y1;
        // Rz
        let (x3, y3) = (x2 * cc - y2 * sc, x2 * sc + y2 * cc);
        *p = [x3, y3, z2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quito_ir::FragmentSpec;

    #[test]
    fn union_of_two_cubes_volume() {
        // two unit cubes overlapping in half -> volume 1.5
        let node = Node::Union(vec![
            Node::Cube { size: [1.0, 1.0, 1.0], center: false },
            Node::Translate {
                v: [0.5, 0.0, 0.0],
                child: Box::new(Node::Cube { size: [1.0, 1.0, 1.0], center: false }),
            },
        ]);
        let m = render(&node).unwrap();
        assert!((m.volume() - 1.5).abs() < 1e-6, "vol = {}", m.volume());
    }

    #[test]
    fn difference_hole() {
        // 20mm cube minus a through cylinder
        let node = Node::Difference(vec![
            Node::Cube { size: [20.0, 20.0, 20.0], center: true },
            Node::Cylinder {
                h: 40.0,
                r1: 5.0,
                r2: 5.0,
                center: true,
                frags: FragmentSpec { fn_: 64.0, fa: 12.0, fs: 2.0 },
            },
        ]);
        let m = render(&node).unwrap();
        let expected = 8000.0 - std::f64::consts::PI * 25.0 * 20.0;
        let rel = (m.volume() - expected).abs() / expected;
        assert!(rel < 0.01, "difference volume off by {rel}, vol={}", m.volume());
    }

    #[test]
    fn intersection_box_sphere() {
        let node = Node::Intersection(vec![
            Node::Cube { size: [10.0, 10.0, 10.0], center: true },
            Node::Sphere { r: 6.0, frags: FragmentSpec { fn_: 64.0, fa: 12.0, fs: 2.0 } },
        ]);
        let m = render(&node).unwrap();
        assert!(m.volume() > 0.0);
        // intersection is smaller than the cube
        assert!(m.volume() < 1000.0);
    }

    #[test]
    fn linear_extrude_square() {
        let node = Node::LinearExtrude {
            height: 10.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(Node::Square { size: [4.0, 6.0], center: false }),
        };
        let m = render(&node).unwrap();
        assert!((m.volume() - 240.0).abs() < 1e-6, "vol {}", m.volume());
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn projection_cut_section() {
        // Section a cube at z=0 (translated so z=0 is inside), extrude → prism.
        let node = Node::LinearExtrude {
            height: 3.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(Node::Projection {
                cut: true,
                child: Box::new(Node::Translate {
                    v: [0.0, 0.0, 1.0],
                    child: Box::new(Node::Cube { size: [8.0, 6.0, 10.0], center: true }),
                }),
            }),
        };
        let m = render(&node).unwrap();
        assert!((m.volume() - 144.0).abs() < 1e-6, "projection vol {}", m.volume());
    }

    #[test]
    fn offset_shapes() {
        let frags = FragmentSpec { fn_: 64.0, fa: 12.0, fs: 2.0 };
        let ext = |child| Node::LinearExtrude {
            height: 1.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(child),
        };
        // mitred grow: 10x10 -> 14x14 = 196
        let m = render(&ext(Node::Offset {
            r: 0.0,
            delta: 2.0,
            chamfer: false,
            frags,
            child: Box::new(Node::Square { size: [10.0, 10.0], center: false }),
        }))
        .unwrap();
        assert!((m.volume() - 196.0).abs() < 1e-6, "miter {}", m.volume());
        // inset: 10x10 -> 6x6 = 36
        let m = render(&ext(Node::Offset {
            r: -2.0,
            delta: 0.0,
            chamfer: false,
            frags,
            child: Box::new(Node::Square { size: [10.0, 10.0], center: false }),
        }))
        .unwrap();
        assert!((m.volume() - 36.0).abs() < 1e-6, "inset {}", m.volume());
    }

    /// Total (unsigned) area of a flat 2D mesh at z=0.
    fn flat_area(m: &Mesh) -> f64 {
        m.tris
            .iter()
            .map(|t| {
                let p = |i: u32| m.verts[i as usize];
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
            })
            .sum()
    }

    #[test]
    fn bare_2d_difference() {
        // square(10) minus a 4×4 square hole → flat area 100 − 16 = 84 (the 3D
        // kernel would produce garbage on these coplanar flat meshes).
        let node = Node::Difference(vec![
            Node::Square { size: [10.0, 10.0], center: false },
            Node::Translate {
                v: [3.0, 3.0, 0.0],
                child: Box::new(Node::Square { size: [4.0, 4.0], center: false }),
            },
        ]);
        let m = render(&node).unwrap();
        assert!((flat_area(&m) - 84.0).abs() < 1e-6, "area {}", flat_area(&m));
    }

    #[test]
    fn bare_2d_intersection_and_union() {
        let sq = |x: f64, y: f64| Node::Translate {
            v: [x, y, 0.0],
            child: Box::new(Node::Square { size: [10.0, 10.0], center: false }),
        };
        // Overlap of two squares offset by 5 → 5×5 = 25.
        let inter = Node::Intersection(vec![sq(0.0, 0.0), sq(5.0, 5.0)]);
        assert!((flat_area(&render(&inter).unwrap()) - 25.0).abs() < 1e-6);
        // Union of the same two → 200 − 25 overlap = 175.
        let uni = Node::Union(vec![sq(0.0, 0.0), sq(5.0, 5.0)]);
        assert!((flat_area(&render(&uni).unwrap()) - 175.0).abs() < 1e-6);
    }

    #[test]
    fn projection_silhouette() {
        // projection(cut=false) of a 10×20×30 box → its 10×20 footprint (200).
        let node = Node::Projection {
            cut: false,
            child: Box::new(Node::Cube { size: [10.0, 20.0, 30.0], center: false }),
        };
        let m = render(&node).unwrap();
        assert!((flat_area(&m) - 200.0).abs() < 1e-3, "area {}", flat_area(&m));
    }

    #[test]
    fn extrude_polygon_with_hole() {
        // A 10×10 square with a centered 4×4 hole (even-odd), extruded 1 mm →
        // volume 100 − 16 = 84. Exercises the earcut hole triangulation.
        let points = vec![
            [0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], // outer
            [3.0, 3.0], [3.0, 7.0], [7.0, 7.0], [7.0, 3.0], // hole
        ];
        let paths = Some(vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7]]);
        let node = Node::LinearExtrude {
            height: 1.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(Node::Polygon { points, paths }),
        };
        let m = render(&node).unwrap();
        assert!((m.volume() - 84.0).abs() < 1e-6, "vol {}", m.volume());
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn minkowski_rounded_cube() {
        let frags = FragmentSpec { fn_: 24.0, fa: 12.0, fs: 2.0 };
        let node = Node::Minkowski(vec![
            Node::Cube { size: [10.0, 10.0, 10.0], center: true },
            Node::Sphere { r: 2.0, frags },
        ]);
        let m = render(&node).unwrap();
        // matches OpenSCAD (~2592.88); allow small tessellation tolerance
        assert!((m.volume() - 2592.88).abs() < 5.0, "minkowski vol {}", m.volume());
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn extrude_2d_difference() {
        // linear_extrude of (square - circle) = a plate with a hole.
        let frags = FragmentSpec { fn_: 64.0, fa: 12.0, fs: 2.0 };
        let node = Node::LinearExtrude {
            height: 5.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(Node::Difference(vec![
                Node::Square { size: [20.0, 20.0], center: true },
                Node::Circle { r: 5.0, frags },
            ])),
        };
        let m = render(&node).unwrap();
        let expected = (400.0 - std::f64::consts::PI * 25.0) * 5.0;
        let rel = (m.volume() - expected).abs() / expected;
        assert!(rel < 0.01, "plate vol off by {rel}: {}", m.volume());
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn rotate_extrude_torus() {
        // circle r=2 at radius 10 revolved -> torus, volume 2*pi^2*R*r^2.
        let frags = FragmentSpec { fn_: 64.0, fa: 12.0, fs: 2.0 };
        let node = Node::RotateExtrude {
            angle: 360.0,
            frags,
            child: Box::new(Node::Translate {
                v: [10.0, 0.0, 0.0],
                child: Box::new(Node::Circle { r: 2.0, frags }),
            }),
        };
        let m = render(&node).unwrap();
        let expected = 2.0 * std::f64::consts::PI.powi(2) * 10.0 * 4.0;
        let rel = (m.volume() - expected).abs() / expected;
        assert!(rel < 0.01, "torus vol off by {rel}: {}", m.volume());
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn cache_matches_cold_and_reuses() {
        let frags = FragmentSpec { fn_: 32.0, fa: 12.0, fs: 2.0 };
        let model = |r: f64| {
            Node::Difference(vec![
                Node::Cube { size: [20.0, 20.0, 20.0], center: true },
                Node::Sphere { r, frags },
            ])
        };
        let kernel = ManifoldKernel::new();
        let mut cache = GeomCache::new();

        // Warm render matches a cold render.
        let cold = render_with(&model(8.0), &kernel).unwrap();
        let warm = render_cached(&model(8.0), &kernel, &mut cache).unwrap();
        assert!((cold.volume() - warm.volume()).abs() < 1e-6);
        let after_first = cache.len();
        assert!(after_first > 0);

        // Re-rendering the identical tree adds nothing (pure cache hits).
        let again = render_cached(&model(8.0), &kernel, &mut cache).unwrap();
        assert!((again.volume() - warm.volume()).abs() < 1e-6);
        assert_eq!(cache.len(), after_first, "identical re-render should not grow cache");

        // A warm edit (changed radius) reuses the unchanged cube leaf: only the
        // sphere and the difference are new, so the cache grows by exactly 2.
        let edited = render_cached(&model(7.0), &kernel, &mut cache).unwrap();
        let cold_edit = render_with(&model(7.0), &kernel).unwrap();
        assert!((edited.volume() - cold_edit.volume()).abs() < 1e-6);
        assert_eq!(cache.len(), after_first + 2, "warm edit should reuse the cube leaf");
    }

    #[test]
    fn cache_cse_dedups_identical_subtrees() {
        // Two identical spheres in a union hash the same → rendered once.
        let frags = FragmentSpec { fn_: 32.0, fa: 12.0, fs: 2.0 };
        let node = Node::Union(vec![
            Node::Translate {
                v: [0.0, 0.0, 0.0],
                child: Box::new(Node::Sphere { r: 5.0, frags }),
            },
            Node::Translate {
                v: [20.0, 0.0, 0.0],
                child: Box::new(Node::Sphere { r: 5.0, frags }),
            },
        ]);
        let kernel = ManifoldKernel::new();
        let mut cache = GeomCache::new();
        render_cached(&node, &kernel, &mut cache).unwrap();
        // Entries: 1 sphere (shared), 2 translates (distinct v), 1 union = 4.
        assert_eq!(cache.len(), 4, "identical spheres should share one cache entry");
    }

    /// Bake-off: the pure-Rust boolmesh kernel must agree with the C++ Manifold
    /// kernel to within tolerance on a mixed union/difference/intersection model.
    #[test]
    fn kernels_agree() {
        let frags = FragmentSpec { fn_: 48.0, fa: 12.0, fs: 2.0 };
        let cases = [
            Node::Union(vec![
                Node::Cube { size: [10.0, 10.0, 10.0], center: true },
                Node::Sphere { r: 6.5, frags },
            ]),
            Node::Difference(vec![
                Node::Cube { size: [20.0, 20.0, 20.0], center: true },
                Node::Cylinder { h: 40.0, r1: 5.0, r2: 5.0, center: true, frags },
            ]),
            Node::Intersection(vec![
                Node::Cube { size: [10.0, 10.0, 10.0], center: true },
                Node::Sphere { r: 6.0, frags },
            ]),
        ];
        let cpp = ManifoldKernel::new();
        let rs = BoolmeshKernel::new();
        for (i, node) in cases.iter().enumerate() {
            let a = render_with(node, &cpp).unwrap();
            let b = render_with(node, &rs).unwrap();
            let rel = (a.volume() - b.volume()).abs() / a.volume().max(1e-9);
            assert!(
                rel < 0.005,
                "case {i}: kernels disagree: cpp={} boolmesh={} (Δ={rel})",
                a.volume(),
                b.volume()
            );
            assert!(b.signed_volume() > 0.0, "case {i}: boolmesh output inward-facing");
        }
    }
}
