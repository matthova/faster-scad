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

fn render_scad_src(src: &str, dir: &str) -> quito_geom::Mesh {
    struct R(String);
    impl quito_eval::FileResolver for R {
        fn load(&self, path: &str, _from: &str) -> Option<quito_eval::LoadedFile> {
            let p = std::path::Path::new(&self.0).join(path);
            let source = std::fs::read_to_string(&p).ok()?;
            Some(quito_eval::LoadedFile { key: p.to_string_lossy().into(), source, dir: self.0.clone() })
        }
    }
    let prog = quito_syntax::parse(src).expect("parse");
    let out = quito_eval::eval_program_with(&prog, &R(dir.to_string()), dir).expect("eval");
    quito_geom::render(&out.node).expect("render")
}

/// The full parametric lamp *assembly* — hull, rotate_extrude (torus),
/// linear_extrude, polyhedron, color, and nested differences (56 operations).
/// Must render to a positive-volume solid comparable to OpenSCAD's Manifold
/// backend (~36 in³; OpenSCAD's default CGAL backend actually crashes on it).
#[test]
fn renders_lamp_assembly() {
    let src = std::fs::read_to_string(workspace_root().join("examples/lamp.scad"))
        .expect("read lamp")
        .replace("part = \"shade\"", "part = \"assembly\"");
    let dir = workspace_root().to_string_lossy().into_owned();
    let mesh = render_scad_src(&src, &dir);
    assert!(mesh.tris.len() > 20_000, "unexpected tri count {}", mesh.tris.len());
    assert!(
        (mesh.volume() - 36.0).abs() < 2.0,
        "assembly volume {} not ~36",
        mesh.volume()
    );
}

/// BOSL2 (BSD, git submodule) exercises a large slice of the language and its
/// attachment system. `cuboid([20,20,20])` must render a 20 mm cube (vol 8000),
/// proving matrix multiply, `search`, nested `children()`, and `use`-inside-
/// `include` resolution all work together. Skipped if the submodule is absent.
#[test]
fn renders_bosl2_cuboid() {
    let corpus = workspace_root().join("corpus");
    if !corpus.join("BOSL2/std.scad").exists() {
        eprintln!("skipping: corpus/BOSL2 submodule not checked out");
        return;
    }
    struct DR(std::path::PathBuf);
    impl quito_eval::FileResolver for DR {
        fn load(&self, path: &str, from: &str) -> Option<quito_eval::LoadedFile> {
            for c in [std::path::Path::new(from).join(path), self.0.join(path)] {
                if let Ok(source) = std::fs::read_to_string(&c) {
                    let key = std::fs::canonicalize(&c)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| c.to_string_lossy().into_owned());
                    let dir = c
                        .parent()
                        .map(|d| d.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    return Some(quito_eval::LoadedFile { key, source, dir });
                }
            }
            None
        }
    }
    let src = "include <BOSL2/std.scad>\ncuboid([20,20,20]);";
    let prog = quito_syntax::parse(src).expect("parse");
    let out = quito_eval::eval_program_with(&prog, &DR(corpus.clone()), corpus.to_str().unwrap())
        .expect("eval");
    let mesh = quito_geom::render(&out.node).expect("render");
    assert!(
        (mesh.volume() - 8000.0).abs() < 1.0,
        "BOSL2 cuboid volume {} not ~8000",
        mesh.volume()
    );
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
