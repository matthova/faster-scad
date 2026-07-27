//! End-to-end render regression tests: parse -> eval -> geometry, asserting
//! mesh validity on real models.

use std::collections::HashMap;
use std::path::Path;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn render_scad(rel: &str) -> quito_geom::Mesh {
    let src = std::fs::read_to_string(workspace_root().join(rel)).expect("read scad");
    let prog = quito_syntax::parse(&src).expect("parse");
    let out = quito_eval::eval_program(&prog).expect("eval");
    quito_geom::render(&out.node).expect("render")
}

/// Every undirected edge of a closed 2-manifold is shared by exactly two
/// triangles.
fn is_manifold(mesh: &quito_geom::Mesh) -> bool {
    let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
    for t in &mesh.tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let key = if a < b { (a, b) } else { (b, a) };
            *edges.entry(key).or_default() += 1;
        }
    }
    edges.values().all(|&c| c == 2)
}

/// The draped/fluted dome lamp shade: a single analytic-surface polyhedron
/// built with C-style-for comprehensions. Must render to a watertight,
/// outward-facing mesh matching OpenSCAD (17408 triangles, volume ~13.596).
#[test]
fn renders_lamp_shade() {
    let mesh = render_scad("examples/lamp.scad");
    assert_eq!(mesh.tris.len(), 17408, "unexpected triangle count");
    assert!(mesh.signed_volume() > 0.0, "mesh is inward-facing");
    assert!(
        (mesh.volume() - 13.596).abs() < 0.05,
        "volume {} not ~13.596",
        mesh.volume()
    );
    assert!(is_manifold(&mesh), "lamp shade mesh is not 2-manifold");
}
