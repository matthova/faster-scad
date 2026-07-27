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
        let mut acc: Option<boolmesh::prelude::Manifold> = None;
        for m in meshes {
            if m.is_empty() {
                continue;
            }
            let man = bm::to_manifold(&m)?;
            acc = Some(match acc {
                None => man,
                Some(a) => bm::op(&a, &man, bm::Op::Add)?,
            });
        }
        Ok(acc.map(|m| bm::from_manifold(&m)).unwrap_or_default())
    }

    fn difference(&self, base: Mesh, tools: Vec<Mesh>) -> Result<Mesh, GeomError> {
        if base.is_empty() {
            return Ok(Mesh::new());
        }
        let mut acc = bm::to_manifold(&base)?;
        for t in tools {
            if t.is_empty() {
                continue;
            }
            let tm = bm::to_manifold(&t)?;
            acc = bm::op(&acc, &tm, bm::Op::Subtract)?;
        }
        Ok(bm::from_manifold(&acc))
    }

    fn intersection(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError> {
        let mut acc: Option<boolmesh::prelude::Manifold> = None;
        for m in meshes {
            if m.is_empty() {
                return Ok(Mesh::new());
            }
            let man = bm::to_manifold(&m)?;
            acc = Some(match acc {
                None => man,
                Some(a) => bm::op(&a, &man, bm::Op::Intersect)?,
            });
        }
        Ok(acc.map(|m| bm::from_manifold(&m)).unwrap_or_default())
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
            let mut acc: Option<Manifold> = None;
            for m in meshes {
                if m.is_empty() {
                    continue;
                }
                let man = to_manifold(&m)?;
                acc = Some(match acc {
                    None => man,
                    Some(a) => &a + &man,
                });
            }
            Ok(acc.map(|m| from_manifold(&m)).unwrap_or_default())
        }

        fn difference(&self, base: Mesh, tools: Vec<Mesh>) -> Result<Mesh, GeomError> {
            if base.is_empty() {
                return Ok(Mesh::new());
            }
            let mut acc = to_manifold(&base)?;
            for t in tools {
                if t.is_empty() {
                    continue;
                }
                let tm = to_manifold(&t)?;
                acc = &acc - &tm;
            }
            Ok(from_manifold(&acc))
        }

        fn intersection(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError> {
            let mut acc: Option<Manifold> = None;
            for m in meshes {
                if m.is_empty() {
                    return Ok(Mesh::new());
                }
                let man = to_manifold(&m)?;
                acc = Some(match acc {
                    None => man,
                    Some(a) => &a ^ &man,
                });
            }
            Ok(acc.map(|m| from_manifold(&m)).unwrap_or_default())
        }
    }
}
