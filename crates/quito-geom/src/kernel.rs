//! The geometry `Kernel` trait and its backends.
//!
//! M0 ships one backend: `ManifoldKernel`, wrapping the C++ Manifold library
//! via the `manifold-csg` bindings. The trait is the seam for the M0–M2
//! kernel bake-off (pure-Rust `manifold-rust` vs C++ Manifold) called for in
//! the plan; swapping backends means implementing this trait.

use crate::mesh::Mesh;
use crate::GeomError;
use manifold_csg::{Manifold, MeshGL64};

/// A constructive-solid-geometry kernel over triangle meshes.
pub trait Kernel {
    fn union(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError>;
    fn difference(&self, base: Mesh, tools: Vec<Mesh>) -> Result<Mesh, GeomError>;
    fn intersection(&self, meshes: Vec<Mesh>) -> Result<Mesh, GeomError>;
}

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
    let verts: Vec<[f64; 3]> = props
        .chunks(np)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
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
