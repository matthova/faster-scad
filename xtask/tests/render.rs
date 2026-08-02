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
            Some(quito_eval::LoadedFile {
                key: p.to_string_lossy().into(),
                source,
                dir: self.0.clone(),
            })
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
    assert!(
        mesh.tris.len() > 20_000,
        "unexpected tri count {}",
        mesh.tris.len()
    );
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
/// Render a snippet with BOSL2 available (from the corpus submodule), or `None`
/// if the submodule isn't checked out.
fn render_with_bosl2(body: &str) -> Option<quito_geom::Mesh> {
    let corpus = workspace_root().join("corpus");
    if !corpus.join("BOSL2/std.scad").exists() {
        eprintln!("skipping: corpus/BOSL2 submodule not checked out");
        return None;
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
    let src = format!("include <BOSL2/std.scad>\n{body}");
    let prog = quito_syntax::parse(&src).expect("parse");
    let out = quito_eval::eval_program_with(&prog, &DR(corpus.clone()), corpus.to_str().unwrap())
        .expect("eval");
    Some(quito_geom::render(&out.node).expect("render"))
}

/// BOSL2 exercises a large slice of the language + its attachment system.
/// `cuboid([20,20,20])` must be a 20 mm cube (vol 8000) — proving matrix
/// multiply, `search`, nested `children()`, and `use`-inside-`include` all work.
#[test]
fn renders_bosl2_cuboid() {
    let Some(mesh) = render_with_bosl2("cuboid([20,20,20]);") else {
        return;
    };
    assert!(
        (mesh.volume() - 8000.0).abs() < 1.0,
        "cuboid volume {}",
        mesh.volume()
    );
}

/// A *rounded* cuboid exercises range indexing (BOSL2's `is_range`) and the
/// rounding construction; its volume matches OpenSCAD (~7244 at $fn=32).
#[test]
fn renders_bosl2_rounded_cuboid() {
    let Some(mesh) = render_with_bosl2("cuboid([20,20,20], rounding=4, $fn=32);") else {
        return;
    };
    assert!(
        (mesh.volume() - 7244.0).abs() < 25.0,
        "rounded cuboid volume {} not ~7244",
        mesh.volume()
    );
}

/// BOSL2 gears.scad: a spur gear meshed with a rack. This exercises
/// `gear_dist(..., teeth2=0)` (the rack center-distance path) with a top-level
/// variable named `circ_pitch` — the exact shape of the gear-train demo. That
/// name used to leak into `circular_pitch()`'s omitted `circ_pitch` parameter
/// through its `assert(...) expr` guard and trip a spurious assertion. The gear
/// alone (spur_gear vol) plus the rack must both render to non-empty solids.
#[test]
fn renders_bosl2_gear_and_rack() {
    let Some(mesh) = render_with_bosl2(
        "include <BOSL2/gears.scad>\n\
         circ_pitch = 9;\n\
         spur_gear(circ_pitch, 11, 6, 3);\n\
         d = gear_dist(circ_pitch = circ_pitch, teeth1 = 11, teeth2 = 0);\n\
         fwd(d) rack(pitch = circ_pitch, teeth = 9, thickness = 6, width = 12, anchor = CENTER, orient = BACK);",
    ) else {
        return;
    };
    assert!(
        mesh.volume() > 1000.0,
        "gear+rack volume {} unexpectedly small",
        mesh.volume()
    );
}

/// `surface()` builds a heightmap solid (top follows the data, bottom flat).
/// A 3×4 ramp has volume 21 (sum of cell-average heights) and must be
/// outward-facing.
#[test]
fn renders_surface() {
    struct R;
    impl quito_eval::FileResolver for R {
        fn load(&self, path: &str, _from: &str) -> Option<quito_eval::LoadedFile> {
            (path == "h.dat").then(|| quito_eval::LoadedFile {
                key: path.into(),
                source: "1 2 3 4\n2 3 4 5\n3 4 5 6\n".into(),
                dir: ".".into(),
            })
        }
    }
    let prog = quito_syntax::parse("surface(\"h.dat\");").expect("parse");
    let out = quito_eval::eval_program_with(&prog, &R, ".").expect("eval");
    let mesh = quito_geom::render(&out.node).expect("render");
    assert!(
        (mesh.volume() - 21.0).abs() < 1e-6,
        "surface volume {}",
        mesh.volume()
    );
    assert!(mesh.signed_volume() > 0.0, "surface mesh is inward-facing");
}

/// Importing an OpenSCAD-produced 3MF (a ZIP64 archive with deflate-compressed
/// parts) must reconstruct the mesh. This fixture is `difference(){ cube(20,
/// center=true); sphere(12); }` exported by OpenSCAD; volume ≈ 1683.68.
#[test]
fn imports_openscad_3mf() {
    struct R(Vec<u8>);
    impl quito_eval::FileResolver for R {
        fn load(&self, _p: &str, _f: &str) -> Option<quito_eval::LoadedFile> {
            None
        }
        fn load_bytes(&self, path: &str, _f: &str) -> Option<Vec<u8>> {
            (path == "m.3mf").then(|| self.0.clone())
        }
    }
    let tmf = include_bytes!("fixtures/openscad.3mf").to_vec();
    let prog = quito_syntax::parse("import(\"m.3mf\");").expect("parse");
    let out = quito_eval::eval_program_with(&prog, &R(tmf), ".").expect("eval");
    let mesh = quito_geom::render(&out.node).expect("render");
    assert!(
        mesh.tris.len() > 100,
        "too few triangles: {}",
        mesh.tris.len()
    );
    assert!(
        (mesh.volume() - 1683.68).abs() < 0.1,
        "3mf import volume {}",
        mesh.volume()
    );
}

/// `surface()` on a PNG heightmap: each pixel's Rec.709 luma scales to 0..100,
/// the base drops to min−1 (PNG rule). This 3×2 gray ramp matches OpenSCAD's
/// rendered volume of 121.0196 (verified against the oracle).
#[test]
fn surface_png_heightmap() {
    struct R(Vec<u8>);
    impl quito_eval::FileResolver for R {
        fn load(&self, _p: &str, _f: &str) -> Option<quito_eval::LoadedFile> {
            None
        }
        fn load_bytes(&self, path: &str, _f: &str) -> Option<Vec<u8>> {
            (path == "h.png").then(|| self.0.clone())
        }
    }
    let png = include_bytes!("fixtures/heightmap.png").to_vec();
    let prog = quito_syntax::parse("surface(\"h.png\");").expect("parse");
    let out = quito_eval::eval_program_with(&prog, &R(png), ".").expect("eval");
    let mesh = quito_geom::render(&out.node).expect("render");
    assert!(
        (mesh.volume() - 121.0196).abs() < 1e-3,
        "png surface volume {}",
        mesh.volume()
    );
    assert!(mesh.signed_volume() > 0.0, "png surface is inward-facing");
}

/// A DXF profile imported and extruded must produce the extruded area×height.
/// A 10×20 square with a 2×2 hole (LWPOLYLINEs) → area 196, ×3 → volume 588.
#[test]
fn imports_and_extrudes_dxf() {
    struct R(Vec<u8>);
    impl quito_eval::FileResolver for R {
        fn load(&self, _p: &str, _f: &str) -> Option<quito_eval::LoadedFile> {
            None
        }
        fn load_bytes(&self, path: &str, _f: &str) -> Option<Vec<u8>> {
            (path == "p.dxf").then(|| self.0.clone())
        }
    }
    let outer = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 20.0], [0.0, 20.0]];
    let hole = vec![[4.0, 4.0], [4.0, 6.0], [6.0, 6.0], [6.0, 4.0]];
    let dxf = quito_geom::export_dxf(&[outer, hole]).into_bytes();
    let prog = quito_syntax::parse("linear_extrude(3) import(\"p.dxf\");").expect("parse");
    let out = quito_eval::eval_program_with(&prog, &R(dxf), ".").expect("eval");
    let mesh = quito_geom::render(&out.node).expect("render");
    assert!(
        (mesh.volume() - 588.0).abs() < 1e-3,
        "dxf extrude volume {}",
        mesh.volume()
    );
    assert!(mesh.signed_volume() > 0.0, "dxf extrude is inward-facing");
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

/// Transform argument binding (A1), checked against OpenSCAD 2024.12.
///
/// - Named args must actually transform the child (the old `first_positional`
///   path silently ignored them, leaving `cube` at the origin).
/// - `rotate(a, v)` axis-angle must rotate about the given axis, not treat the
///   angle as an Euler `[a,0,0]`. A 10 mm cube rotated 45° about `[1,1,0]` has a
///   distinctive bbox (±7.5 in X/Y, ±8.5355 in Z) that Euler rotation misses.
#[test]
fn transforms_named_and_axis_angle() {
    let dir = workspace_root().to_string_lossy().into_owned();
    let bbox = |m: &quito_geom::Mesh| m.bbox().expect("non-empty mesh");
    let close = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3);

    // Named translate: the child moves to x = 100..110.
    let m = render_scad_src("translate(v=[100,0,0]) cube(10);", &dir);
    let (lo, hi) = bbox(&m);
    assert!(
        close(lo, [100.0, 0.0, 0.0]) && close(hi, [110.0, 10.0, 10.0]),
        "translate(v=) bbox {lo:?}..{hi:?}"
    );

    // Axis-angle, positional and named, must match OpenSCAD's bbox and volume.
    for src in [
        "rotate(45,[1,1,0]) cube(10, center=true);",
        "rotate(a=45, v=[1,1,0]) cube(10, center=true);",
    ] {
        let m = render_scad_src(src, &dir);
        assert!(
            (m.volume() - 1000.0).abs() < 0.1,
            "{src}: volume {}",
            m.volume()
        );
        let (lo, hi) = bbox(&m);
        assert!(
            close(lo, [-7.5, -7.5, -8.5355]) && close(hi, [7.5, 7.5, 8.5355]),
            "{src}: bbox {lo:?}..{hi:?}"
        );
    }
}
