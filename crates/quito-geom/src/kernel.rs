//! The geometry `Kernel` trait and its backends.
//!
//! The kernel bake-off (per the plan) is realized as two backends behind one
//! trait:
//!
//! * [`ManifoldKernel`] — the C++ Manifold library via `manifold-csg`. Native
//!   only; battle-tested and fast, but does not target
//!   `wasm32-unknown-unknown`.
//! * [`RustManifoldKernel`] — the pure-Rust port of Manifold. Builds
//!   everywhere, including wasm, and is the default backend in the browser.
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
// Pure-Rust backend: Manifold
// ===================================================================

/// Pure-Rust Manifold CSG backend. Available on all targets.
#[derive(Default)]
pub struct RustManifoldKernel;

impl RustManifoldKernel {
    pub fn new() -> Self {
        RustManifoldKernel
    }
}

/// Backward-compatible name for the former browser kernel.
pub type BoolmeshKernel = RustManifoldKernel;

mod rust_manifold {
    use super::*;
    use manifold_rust::manifold::Manifold;
    use manifold_rust::types::{Error, MeshGL64};

    pub(super) fn to_manifold(m: &Mesh) -> Result<Manifold, GeomError> {
        let mut vert_properties = Vec::with_capacity(m.verts.len() * 3);
        for v in &m.verts {
            vert_properties.extend_from_slice(v);
        }
        let tri_verts = m
            .tris
            .iter()
            .flat_map(|t| [t[0] as u64, t[1] as u64, t[2] as u64])
            .collect();
        let mesh = MeshGL64 {
            num_prop: 3,
            vert_properties,
            tri_verts,
            ..MeshGL64::default()
        };
        let man = Manifold::from_mesh_gl64(&mesh);
        check(man)
    }

    pub(super) fn from_manifold(man: &Manifold) -> Mesh {
        let mesh = man.get_mesh_gl64(-1);
        let num_prop = (mesh.num_prop as usize).max(3);
        let verts = mesh
            .vert_properties
            .chunks(num_prop)
            .map(|p| [p[0], p[1], p[2]])
            .collect();
        let tris = mesh
            .tri_verts
            .chunks(3)
            .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
            .collect();
        Mesh { verts, tris }
    }

    pub(super) fn check(man: Manifold) -> Result<Manifold, GeomError> {
        match man.status() {
            Error::NoError => Ok(man),
            status => Err(GeomError::Kernel(status.to_string())),
        }
    }
}

impl Kernel for RustManifoldKernel {
    fn union(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError> {
        let mans = meshes
            .iter()
            .filter(|m| !m.is_empty())
            .map(rust_manifold::to_manifold)
            .collect::<Result<Vec<_>, _>>()?;
        let result = balanced_reduce(mans, |a, b| rust_manifold::check(a.union(b)))?;
        Ok(result
            .map(|m| rust_manifold::from_manifold(&m))
            .unwrap_or_default())
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
            .map(rust_manifold::to_manifold)
            .collect::<Result<Vec<_>, _>>()?;
        let base_man = rust_manifold::to_manifold(&base)?;
        let result = match balanced_reduce(tool_mans, |a, b| {
            rust_manifold::check(a.union(b))
        })? {
            None => base_man,
            Some(tools_union) => rust_manifold::check(base_man.difference(&tools_union))?,
        };
        Ok(rust_manifold::from_manifold(&result))
    }

    fn intersection(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError> {
        if meshes.is_empty() || meshes.iter().any(|m| m.is_empty()) {
            return Ok(Mesh::new());
        }
        let mans = meshes
            .iter()
            .map(rust_manifold::to_manifold)
            .collect::<Result<Vec<_>, _>>()?;
        let result = balanced_reduce(mans, |a, b| {
            rust_manifold::check(a.intersection(b))
        })?;
        Ok(result
            .map(|m| rust_manifold::from_manifold(&m))
            .unwrap_or_default())
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
