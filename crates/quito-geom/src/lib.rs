//! Geometry: mesh types, the fragment formula + primitive tessellation, the
//! `Kernel` trait (CSG boolean backend), and the CSG-tree -> mesh renderer.

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

#[derive(Debug, thiserror::Error)]
pub enum GeomError {
    #[error("kernel error: {0}")]
    Kernel(String),
    #[error("input geometry is not manifold: {0}")]
    NonManifold(String),
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

/// Render a CSG tree to a mesh using the given kernel.
pub fn render_with(node: &Node, kernel: &dyn Kernel) -> Result<Mesh, GeomError> {
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

        // 2D shapes rendered as a flat mesh at z=0.
        Node::Square { .. } | Node::Circle { .. } | Node::Polygon { .. } => {
            Ok(shape2d::flat_mesh(&shape2d::render2d(node)))
        }
        Node::LinearExtrude {
            height,
            center,
            twist,
            scale,
            slices,
            child,
        } => Ok(shape2d::linear_extrude(
            &shape2d::render2d(child),
            *height,
            *center,
            *twist,
            *scale,
            *slices,
        )),
        Node::RotateExtrude { angle, frags, child } => {
            Ok(shape2d::rotate_extrude(&shape2d::render2d(child), *angle, *frags))
        }

        Node::Group(children) => {
            let meshes = render_all(children, kernel)?;
            kernel.union(meshes)
        }
        Node::Union(children) => {
            let meshes = render_all(children, kernel)?;
            kernel.union(meshes)
        }
        Node::Intersection(children) => {
            let meshes = render_all(children, kernel)?;
            kernel.intersection(meshes)
        }
        Node::Difference(children) => {
            let mut meshes = render_all(children, kernel)?;
            if meshes.is_empty() {
                Ok(Mesh::new())
            } else {
                let base = meshes.remove(0);
                kernel.difference(base, meshes)
            }
        }

        Node::Translate { v, child } => {
            let mut m = render_with(child, kernel)?;
            translate(&mut m, *v);
            Ok(m)
        }
        Node::Rotate { deg, child } => {
            let mut m = render_with(child, kernel)?;
            rotate(&mut m, *deg);
            Ok(m)
        }
        Node::Scale { v, child } => {
            let mut m = render_with(child, kernel)?;
            scale(&mut m, *v);
            Ok(m)
        }
    }
}

fn render_all(children: &[Node], kernel: &dyn Kernel) -> Result<Vec<Mesh>, GeomError> {
    children.iter().map(|c| render_with(c, kernel)).collect()
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
