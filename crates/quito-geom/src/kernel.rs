//! The geometry `Kernel` trait and its backends.
//!
//! The kernel bake-off (per the plan) is realized as two backends behind one
//! trait:
//!
//! * [`ManifoldKernel`] — the C++ Manifold library via `manifold-csg`. Native
//!   only; battle-tested and fast, but does not target
//!   `wasm32-unknown-unknown`.
//! * [`BoolmeshKernel`] — the pure-Rust `boolmesh` kernel. Builds everywhere,
//!   including wasm, and is the default backend in the browser.
//!
//! Both are differential-tested against each other on native builds.

use crate::mesh::Mesh;
use crate::GeomError;

/// A constructive-solid-geometry kernel over triangle meshes.
pub trait Kernel {
    fn union(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError>;
    fn difference(&self, base: Mesh, tools: Vec<Mesh>) -> Result<Mesh, GeomError>;
    fn intersection(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError>;
    /// Convex hull of all vertices in the given meshes.
    fn hull(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError>;
}

/// Collect every vertex from a set of meshes.
fn all_points(meshes: &[Mesh]) -> Vec<[f64; 3]> {
    meshes.iter().flat_map(|m| m.verts.iter().copied()).collect()
}

/// Combine items pairwise in a balanced (divide-and-conquer) tree rather than a
/// linear fold. For an associative+commutative op like union/intersection this
/// keeps intermediate operands small — O(log n) boolean "depth" instead of one
/// accumulator that grows with every operand — which is much faster for the
/// many-operand unions typical of `for`-generated geometry.
fn balanced_reduce<T>(
    mut items: Vec<T>,
    combine: impl Fn(&T, &T) -> Result<T, GeomError>,
) -> Result<Option<T>, GeomError> {
    if items.is_empty() {
        return Ok(None);
    }
    while items.len() > 1 {
        let mut next = Vec::with_capacity(items.len().div_ceil(2));
        let mut iter = items.into_iter();
        while let Some(a) = iter.next() {
            match iter.next() {
                Some(b) => next.push(combine(&a, &b)?),
                None => next.push(a),
            }
        }
        items = next;
    }
    Ok(items.into_iter().next())
}

// ===================================================================
// Pure-Rust backend: boolmesh
// ===================================================================

/// Pure-Rust CSG backend (`boolmesh`). Available on all targets.
#[derive(Default)]
pub struct BoolmeshKernel;

impl BoolmeshKernel {
    pub fn new() -> Self {
        BoolmeshKernel
    }
}

mod bm {
    use super::*;
    use boolmesh::prelude::{compute_boolean, Manifold, OpType};

    pub(super) fn to_manifold(m: &Mesh) -> Result<Manifold, GeomError> {
        let mut pos: Vec<f64> = Vec::with_capacity(m.verts.len() * 3);
        for v in &m.verts {
            pos.extend_from_slice(v);
        }
        let idx: Vec<usize> = m
            .tris
            .iter()
            .flat_map(|t| [t[0] as usize, t[1] as usize, t[2] as usize])
            .collect();
        Manifold::new(&pos, &idx).map_err(GeomError::Kernel)
    }

    pub(super) fn from_manifold(man: &Manifold) -> Mesh {
        let verts: Vec<[f64; 3]> = man.ps.iter().map(|p| [p.x, p.y, p.z]).collect();
        let tris: Vec<[u32; 3]> = man
            .hs
            .chunks(3)
            .map(|c| [c[0].tail as u32, c[1].tail as u32, c[2].tail as u32])
            .collect();
        Mesh { verts, tris }
    }

    pub(super) fn op(a: &Manifold, b: &Manifold, op: OpType) -> Result<Manifold, GeomError> {
        compute_boolean(a, b, op).map_err(GeomError::Kernel)
    }

    pub(super) use boolmesh::prelude::OpType as Op;
}

impl Kernel for BoolmeshKernel {
    fn union(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError> {
        let mans = meshes
            .iter()
            .filter(|m| !m.is_empty())
            .map(bm::to_manifold)
            .collect::<Result<Vec<_>, _>>()?;
        let r = balanced_reduce(mans, |a, b| bm::op(a, b, bm::Op::Add))?;
        Ok(r.map(|m| bm::from_manifold(&m)).unwrap_or_default())
    }

    fn difference(&self, base: Mesh, tools: Vec<Mesh>) -> Result<Mesh, GeomError> {
        if base.is_empty() {
            return Ok(Mesh::new());
        }
        // base - t1 - t2 - ... == base - (t1 ∪ t2 ∪ ...): union the tools once
        // (balanced) then a single subtraction, instead of N subtractions that
        // each re-process the whole base.
        let tool_mans = tools
            .iter()
            .filter(|t| !t.is_empty())
            .map(bm::to_manifold)
            .collect::<Result<Vec<_>, _>>()?;
        let base_man = bm::to_manifold(&base)?;
        let result = match balanced_reduce(tool_mans, |a, b| bm::op(a, b, bm::Op::Add))? {
            None => base_man,
            Some(tools_union) => bm::op(&base_man, &tools_union, bm::Op::Subtract)?,
        };
        Ok(bm::from_manifold(&result))
    }

    fn intersection(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError> {
        if meshes.is_empty() || meshes.iter().any(|m| m.is_empty()) {
            return Ok(Mesh::new());
        }
        let mans = meshes.iter().map(bm::to_manifold).collect::<Result<Vec<_>, _>>()?;
        let r = balanced_reduce(mans, |a, b| bm::op(a, b, bm::Op::Intersect))?;
        Ok(r.map(|m| bm::from_manifold(&m)).unwrap_or_default())
    }

    fn hull(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError> {
        Ok(crate::hull::convex_hull(&all_points(&meshes)))
    }
}

// ===================================================================
// C++ backend: manifold-csg (native only)
// ===================================================================

#[cfg(not(target_arch = "wasm32"))]
pub use manifold_backend::ManifoldKernel;

#[cfg(not(target_arch = "wasm32"))]
mod manifold_backend {
    use super::*;
    use manifold_csg::{Manifold, MeshGL64};

    /// C++ Manifold backend.
    #[derive(Default)]
    pub struct ManifoldKernel;

    impl ManifoldKernel {
        pub fn new() -> Self {
            ManifoldKernel
        }
    }

    fn to_manifold(m: &Mesh) -> Result<Manifold, GeomError> {
        let mut props: Vec<f64> = Vec::with_capacity(m.verts.len() * 3);
        for v in &m.verts {
            props.extend_from_slice(v);
        }
        let tris: Vec<u64> = m
            .tris
            .iter()
            .flat_map(|t| [t[0] as u64, t[1] as u64, t[2] as u64])
            .collect();
        let mesh = MeshGL64::new(&props, 3, &tris)
            .map_err(|e| GeomError::Kernel(format!("MeshGL64::new: {e:?}")))?;
        let man = Manifold::from_meshgl64(&mesh)
            .map_err(|e| GeomError::Kernel(format!("from_meshgl64: {e:?}")))?;
        man.status()
            .map_err(|e| GeomError::NonManifold(format!("{e:?}")))?;
        Ok(man)
    }

    fn from_manifold(man: &Manifold) -> Mesh {
        let mg = man.to_meshgl64();
        let np = mg.num_prop().max(3);
        let props = mg.vert_properties();
        let verts: Vec<[f64; 3]> = props.chunks(np).map(|c| [c[0], c[1], c[2]]).collect();
        let tv = mg.tri_verts();
        let tris: Vec<[u32; 3]> = tv
            .chunks(3)
            .map(|c| [c[0] as u32, c[1] as u32, c[2] as u32])
            .collect();
        Mesh { verts, tris }
    }

    impl Kernel for ManifoldKernel {
        fn union(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError> {
            let mans = meshes
                .iter()
                .filter(|m| !m.is_empty())
                .map(to_manifold)
                .collect::<Result<Vec<_>, _>>()?;
            let r = super::balanced_reduce(mans, |a, b| Ok(a + b))?;
            Ok(r.map(|m| from_manifold(&m)).unwrap_or_default())
        }

        fn difference(&self, base: Mesh, tools: Vec<Mesh>) -> Result<Mesh, GeomError> {
            if base.is_empty() {
                return Ok(Mesh::new());
            }
            // base - t1 - t2 - ... == base - (t1 ∪ t2 ∪ ...): one subtraction
            // after a balanced union of the tools.
            let tool_mans = tools
                .iter()
                .filter(|t| !t.is_empty())
                .map(to_manifold)
                .collect::<Result<Vec<_>, _>>()?;
            let base_man = to_manifold(&base)?;
            let result = match super::balanced_reduce(tool_mans, |a, b| Ok(a + b))? {
                None => base_man,
                Some(tools_union) => &base_man - &tools_union,
            };
            Ok(from_manifold(&result))
        }

        fn intersection(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError> {
            if meshes.is_empty() || meshes.iter().any(|m| m.is_empty()) {
                return Ok(Mesh::new());
            }
            let mans = meshes.iter().map(to_manifold).collect::<Result<Vec<_>, _>>()?;
            let r = super::balanced_reduce(mans, |a, b| Ok(a ^ b))?;
            Ok(r.map(|m| from_manifold(&m)).unwrap_or_default())
        }

        fn hull(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError> {
            let points = super::all_points(&meshes);
            if points.len() < 4 {
                return Ok(Mesh::new());
            }
            Ok(from_manifold(&Manifold::hull_pts(&points)))
        }
    }
}
