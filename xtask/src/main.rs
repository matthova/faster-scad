//! Oracle harness for Quito.
//!
//! `cargo run -p xtask -- bless-echo`  — regenerate echo goldens from the
//!                                       installed OpenSCAD binary.
//! `cargo run -p xtask -- echo`        — run quito against the committed echo
//!                                       goldens and report a pass rate.
//!
//! The echo oracle is the executable spec for the interpreter (language dark
//! corners). Goldens are captured with
//! `openscad --export-format=echo -o - <file>` (no geometry render).

use quito_geom::Mesh;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "echo".into());
    let root = workspace_root();
    let cases = root.join("corpus/echo");
    let goldens = root.join("corpus/golden/echo");

    match mode.as_str() {
        "bless-echo" => bless_echo(&cases, &goldens),
        "echo" => {
            let ok = check_echo(&cases, &goldens);
            if !ok {
                std::process::exit(1);
            }
        }
        "bless-geom" => bless_geom(&root.join("corpus/geom"), &root.join("corpus/golden/geom")),
        "geom" => {
            if !check_geom(&root.join("corpus/geom"), &root.join("corpus/golden/geom")) {
                std::process::exit(1);
            }
        }
        "bosl2" => {
            if !run_bosl2(&root) {
                std::process::exit(1);
            }
        }
        "bench" => run_bench(&root),
        other => {
            eprintln!("unknown command: {other}");
            eprintln!("usage: xtask [bless-echo|echo|bless-geom|geom|bosl2|bench]");
            std::process::exit(2);
        }
    }
}

/// Run BOSL2's function-oriented test suite (BSD-licensed submodule) through
/// quito and report the pass rate — the M2 exit metric.
///
/// Returns `true` only if every listed test file exists, parses, evaluates
/// without error, and actually executed at least one `assert()` (a test that
/// runs zero assertions is a vacuous pass and is rejected). A missing file or
/// an empty subset is a hard failure — this is what makes the check meaningful
/// in CI, where the submodule not being checked out would otherwise report
/// 0/0 and exit 0.
fn run_bosl2(root: &Path) -> bool {
    let dir = root.join("corpus/BOSL2/tests");
    // Function-oriented subset (M2). `test_quaternions` is intentionally absent:
    // it does not exist in the pinned BOSL2 submodule. Adding a name here that
    // has no `.scadtest` file is now a hard failure rather than a silent skip.
    let subset = [
        "test_math",
        "test_lists",
        "test_comparisons",
        "test_strings",
        "test_vectors",
        "test_linalg",
        "test_trigonometry",
        "test_utility",
        "test_fnliterals",
        "test_structs",
        "test_coords",
        "test_affine",
        "test_geometry",
        "test_paths",
        "test_regions",
    ];
    let dir_str = dir.to_string_lossy().into_owned();
    let mut pass = 0;
    let mut total = 0;
    let mut missing = Vec::new();
    let mut failed: Vec<(&str, String)> = Vec::new();

    for name in subset {
        let path = dir.join(format!("{name}.scadtest"));
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(e) => {
                // Missing/unreadable file: hard failure, not a silent skip.
                missing.push((name, e.to_string()));
                continue;
            }
        };
        let Some(script) = extract_script(&raw) else {
            failed.push((name, "no `script = '''...'''` block".into()));
            total += 1;
            continue;
        };
        total += 1;
        let result = match quito_syntax::parse(&script) {
            Ok(prog) => match quito_eval::eval_program_with(&prog, &DiskResolver, &dir_str) {
                Ok(out) if out.asserts_run > 0 => Ok(()),
                Ok(_) => Err("evaluated but ran zero asserts (vacuous)".to_string()),
                Err(e) => Err(format!("eval error: {}", e.0)),
            },
            Err(e) => Err(format!("parse error: {}", e.message)),
        };
        match result {
            Ok(()) => {
                pass += 1;
                println!("  PASS {name}");
            }
            Err(reason) => {
                println!("  FAIL {name} ({reason})");
                failed.push((name, reason));
            }
        }
    }

    for (name, e) in &missing {
        println!("  MISSING {name}.scadtest ({e})");
    }

    let pct = if total == 0 {
        0.0
    } else {
        pass as f64 / total as f64 * 100.0
    };
    println!("\nBOSL2 function tests: {pass}/{total} ({pct:.0}%)");

    let ok = missing.is_empty() && failed.is_empty() && total > 0;
    if !ok {
        if !missing.is_empty() {
            eprintln!(
                "error: {} test file(s) missing — is the corpus/BOSL2 submodule checked out?",
                missing.len()
            );
        }
        if total == 0 {
            eprintln!("error: no BOSL2 tests executed (0/0 is never a pass)");
        }
    }
    ok
}

/// Dual-baseline benchmark (M3 exit): time the release `quito` binary against
/// OpenSCAD's two backends (CGAL default + Manifold) on a set of pinned models,
/// full process wall-clock, best of N runs. Requires `cargo build --release`
/// first and `openscad` on PATH.
fn run_bench(root: &Path) {
    const RUNS: usize = 3;
    let quito = root.join("target/release/quito");
    if !quito.exists() {
        eprintln!(
            "release binary not found at {} — run `cargo build --release` first",
            quito.display()
        );
        std::process::exit(2);
    }
    let out = std::env::temp_dir().join("quito_bench.stl");
    let models: [(&str, &str); 5] = [
        ("lamp-shade", "examples/lamp.scad"),
        ("booleans", "benches/models/booleans.scad"),
        ("rounded", "benches/models/rounded.scad"),
        ("gears", "benches/models/gears.scad"),
        ("eval-bound", "benches/models/evalbound.scad"),
    ];

    println!("Dual-baseline benchmark — best of {RUNS} runs, full-process wall-clock (ms).\n");
    println!(
        "{:<12} {:>10} {:>12} {:>8} {:>12} {:>8}",
        "model", "quito", "oscad-CGAL", "×", "oscad-Mfld", "×"
    );
    println!("{}", "-".repeat(66));

    for (name, rel) in models {
        let path = root.join(rel);
        let q = bench_cmd(
            quito.to_str().unwrap(),
            &[path.to_str().unwrap(), "-o", out.to_str().unwrap()],
            RUNS,
        );
        let cgal = bench_cmd(
            "openscad",
            &["-o", out.to_str().unwrap(), path.to_str().unwrap()],
            RUNS,
        );
        let mfld = bench_cmd(
            "openscad",
            &[
                "--backend=manifold",
                "-o",
                out.to_str().unwrap(),
                path.to_str().unwrap(),
            ],
            RUNS,
        );
        let fmt = |t: Option<f64>| {
            t.map(|v| format!("{v:.0}"))
                .unwrap_or_else(|| "FAIL".into())
        };
        let speed = |base: Option<f64>| match (base, q) {
            (Some(b), Some(qq)) if qq > 0.0 => format!("{:.1}", b / qq),
            _ => "-".into(),
        };
        println!(
            "{:<12} {:>10} {:>12} {:>8} {:>12} {:>8}",
            name,
            fmt(q),
            fmt(cgal),
            speed(cgal),
            fmt(mfld),
            speed(mfld),
        );
    }
    println!("\n(× = OpenSCAD time / quito time; higher is quito being faster.)");

    warm_edit_bench(root, &models);
}

/// Warm-edit bench (M4 exit): in-process, native kernel, render each model with
/// a fresh cache (cold) then re-render the same tree reusing the cache (warm).
/// The warm number is the floor for an edit that doesn't change geometry — and
/// a real geometry edit re-renders only the changed root-to-leaf path.
fn warm_edit_bench(root: &Path, models: &[(&str, &str)]) {
    println!("\nWarm re-render — in-process, native kernel, cache reused (ms):\n");
    println!(
        "{:<12} {:>10} {:>10} {:>10}",
        "model", "cold", "warm", "speed-up"
    );
    println!("{}", "-".repeat(46));
    let kernel = quito_geom::ManifoldKernel::new();
    for (name, rel) in models {
        let path = root.join(rel);
        let Ok(src) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(prog) = quito_syntax::parse(&src) else {
            continue;
        };
        let dir = path
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default();
        let eval = |()| quito_eval::eval_program_with(&prog, &DiskResolver, &dir).ok();
        let Some(out) = eval(()) else { continue };
        let mut cache = quito_geom::GeomCache::new();
        let t0 = Instant::now();
        let _ = quito_geom::render_cached(&out.node, &kernel, &mut cache);
        let cold = t0.elapsed().as_secs_f64() * 1000.0;
        // A fresh eval yields a structurally identical tree → all cache hits.
        let Some(out2) = eval(()) else { continue };
        let t1 = Instant::now();
        let _ = quito_geom::render_cached(&out2.node, &kernel, &mut cache);
        let warm = t1.elapsed().as_secs_f64() * 1000.0;
        let speedup = if warm > 0.0 {
            format!("{:.0}×", cold / warm)
        } else {
            "-".into()
        };
        println!("{:<12} {:>10.1} {:>10.2} {:>10}", name, cold, warm, speedup);
    }
    println!("\n(warm = unchanged tree, all cache hits — the incremental-edit floor.)");
}

/// Best-of-`runs` full-process wall-clock in ms; `None` if the command fails.
fn bench_cmd(cmd: &str, args: &[&str], runs: usize) -> Option<f64> {
    let mut best: Option<f64> = None;
    for _ in 0..runs {
        let t0 = Instant::now();
        let status = Command::new(cmd).args(args).output();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        match status {
            Ok(o) if o.status.success() => best = Some(best.map_or(ms, |b| b.min(ms))),
            _ => return best, // command missing or errored
        }
    }
    best
}

/// Extract the OpenSCAD source from a BOSL2 `.scadtest` `script = '''...'''` block.
fn extract_script(raw: &str) -> Option<String> {
    let start = raw.find("script = '''")? + "script = '''".len();
    let rest = &raw[start..];
    let end = rest.find("'''")?;
    Some(rest[..end].to_string())
}

fn workspace_root() -> PathBuf {
    // xtask/ -> workspace root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn scad_cases(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "scad"))
        .collect();
    v.sort();
    v
}

/// Capture only the `ECHO:` lines from some console output.
fn echo_lines(s: &str) -> Vec<String> {
    s.lines()
        .filter(|l| l.starts_with("ECHO:") || l.starts_with("WARNING:") || l.starts_with("ERROR:"))
        .filter(|l| l.starts_with("ECHO:"))
        .map(|l| l.to_string())
        .collect()
}

struct DiskResolver;
impl quito_eval::FileResolver for DiskResolver {
    fn load(&self, path: &str, from_dir: &str) -> Option<quito_eval::LoadedFile> {
        let p = Path::new(from_dir).join(path);
        let source = fs::read_to_string(&p).ok()?;
        let key = fs::canonicalize(&p)
            .map(|c| c.to_string_lossy().into_owned())
            .unwrap_or_else(|_| p.to_string_lossy().into_owned());
        let dir = p
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default();
        Some(quito_eval::LoadedFile { key, source, dir })
    }

    /// Raw bytes for `import()` of meshes/2D files (STL/OFF/3MF/DXF/SVG).
    fn load_bytes(&self, path: &str, from_dir: &str) -> Option<Vec<u8>> {
        fs::read(Path::new(from_dir).join(path)).ok()
    }
}

fn quito_echo(case: &Path) -> Vec<String> {
    let src = match fs::read_to_string(case) {
        Ok(s) => s,
        Err(e) => return vec![format!("ERROR: read: {e}")],
    };
    let dir = case
        .parent()
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".into());
    match quito_syntax::parse(&src) {
        Ok(prog) => match quito_eval::eval_program_with(&prog, &DiskResolver, &dir) {
            Ok(out) => out.echoes,
            Err(e) => vec![format!("ERROR: {}", e.0)],
        },
        Err(e) => vec![format!("ERROR: parse: {}", e.message)],
    }
}

fn bless_echo(cases: &Path, goldens: &Path) {
    fs::create_dir_all(goldens).unwrap();
    let mut n = 0;
    for case in scad_cases(cases) {
        let out = Command::new("openscad")
            .args(["--export-format=echo", "-o", "-"])
            .arg(&case)
            .output()
            .expect("failed to run openscad — is it installed and on PATH?");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let golden = echo_lines(&stdout).join("\n");
        let name = case.file_stem().unwrap().to_string_lossy();
        let dst = goldens.join(format!("{name}.txt"));
        fs::write(&dst, format!("{golden}\n")).unwrap();
        n += 1;
    }
    eprintln!("blessed {n} echo goldens into {}", goldens.display());
}

fn check_echo(cases: &Path, goldens: &Path) -> bool {
    let mut pass = 0;
    let mut total = 0;
    let mut failures = Vec::new();

    for case in scad_cases(cases) {
        let name = case.file_stem().unwrap().to_string_lossy().to_string();
        let golden_path = goldens.join(format!("{name}.txt"));
        let Ok(golden) = fs::read_to_string(&golden_path) else {
            eprintln!("  ?  {name}: no golden (run `xtask bless-echo`)");
            continue;
        };
        total += 1;
        let expected: Vec<String> = golden.lines().map(|s| s.to_string()).collect();
        let actual = quito_echo(&case);

        if expected == actual {
            pass += 1;
        } else {
            failures.push((name, expected, actual));
        }
    }

    for (name, expected, actual) in &failures {
        println!("FAIL {name}");
        let max = expected.len().max(actual.len());
        for i in 0..max {
            let e = expected.get(i).map(String::as_str).unwrap_or("<none>");
            let a = actual.get(i).map(String::as_str).unwrap_or("<none>");
            if e != a {
                println!("   - openscad: {e}");
                println!("   + quito:    {a}");
            }
        }
    }

    let pct = if total == 0 {
        0.0
    } else {
        pass as f64 / total as f64 * 100.0
    };
    println!("\necho oracle: {pass}/{total} passed ({pct:.0}%)");
    pass == total
}

// ---- geometry oracle --------------------------------------------------
//
// `bless-geom` renders each `corpus/geom/*.scad` with OpenSCAD 2024.12 (dev
// machine, clean-room: binary mesh output only), computes tolerance-based
// metrics, and writes `corpus/golden/geom/<case>.txt`. `geom` renders each case
// with quito's native pipeline, computes the *same* metrics, and diffs them
// against the committed golden — no OpenSCAD needed, so it runs in CI.

/// Default volume tolerance (±0.1%) and its absolute floor for near-empty solids.
const VOL_REL: f64 = 0.001;
const VOL_ABS: f64 = 1e-6;
/// Default bbox / centroid tolerance (±0.01 mm).
const BBOX_ABS: f64 = 0.01;

/// Tolerance-based mesh metrics, computed identically for OpenSCAD's exported
/// STL and quito's native render so the two are directly comparable.
struct GeomMetrics {
    volume: f64,
    bbox: Option<([f64; 3], [f64; 3])>,
    centroid: Option<[f64; 3]>,
    components: usize,
    manifold: bool,
    tris: usize,
}

/// Per-case comparison knobs, parsed from `// oracle: …` comments in the `.scad`.
struct Directives {
    pin_tris: bool,
    vol_tol: f64,
    bbox_tol: f64,
}

fn parse_directives(src: &str) -> Directives {
    let mut d = Directives { pin_tris: false, vol_tol: VOL_REL, bbox_tol: BBOX_ABS };
    for line in src.lines() {
        let Some(rest) = line.trim().strip_prefix("// oracle:") else { continue };
        let toks: Vec<&str> = rest.split_whitespace().collect();
        let mut i = 0;
        while i < toks.len() {
            match toks[i] {
                "tris" => {
                    d.pin_tris = true;
                    i += 1;
                }
                "vol-tol" => {
                    if let Some(v) = toks.get(i + 1).and_then(|s| s.parse().ok()) {
                        d.vol_tol = v;
                    }
                    i += 2;
                }
                "bbox-tol" => {
                    if let Some(v) = toks.get(i + 1).and_then(|s| s.parse().ok()) {
                        d.bbox_tol = v;
                    }
                    i += 2;
                }
                _ => i += 1,
            }
        }
    }
    d
}

fn metrics(mesh: &Mesh) -> GeomMetrics {
    let (verts, tris) = weld(mesh);
    GeomMetrics {
        volume: mesh.volume(),
        bbox: mesh.bbox(),
        centroid: centroid(mesh),
        components: components(&verts, &tris),
        manifold: is_manifold(&tris),
        tris: mesh.tris.len(),
    }
}

/// Dedup vertices by rounded position (1e-6, mirroring `Mesh::from_stl`) and drop
/// triangles that become degenerate, so manifold/component metrics are
/// implementation-agnostic across OpenSCAD's triangle soup and quito's mesh.
fn weld(mesh: &Mesh) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
    use std::collections::HashMap;
    let key = |p: [f64; 3]| {
        [
            (p[0] * 1e6).round() as i64,
            (p[1] * 1e6).round() as i64,
            (p[2] * 1e6).round() as i64,
        ]
    };
    let mut map: HashMap<[i64; 3], u32> = HashMap::new();
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut remap = vec![0u32; mesh.verts.len()];
    for (i, &p) in mesh.verts.iter().enumerate() {
        let id = *map.entry(key(p)).or_insert_with(|| {
            verts.push(p);
            (verts.len() - 1) as u32
        });
        remap[i] = id;
    }
    let mut tris = Vec::with_capacity(mesh.tris.len());
    for t in &mesh.tris {
        let nt = [
            remap[t[0] as usize],
            remap[t[1] as usize],
            remap[t[2] as usize],
        ];
        if nt[0] != nt[1] && nt[1] != nt[2] && nt[0] != nt[2] {
            tris.push(nt);
        }
    }
    (verts, tris)
}

/// A closed 2-manifold: every undirected edge is shared by exactly two triangles.
/// An empty mesh is vacuously manifold.
fn is_manifold(tris: &[[u32; 3]]) -> bool {
    use std::collections::HashMap;
    if tris.is_empty() {
        return true;
    }
    let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
    for t in tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let k = if a < b { (a, b) } else { (b, a) };
            *edges.entry(k).or_default() += 1;
        }
    }
    edges.values().all(|&c| c == 2)
}

fn uf_find(parent: &mut [u32], x: u32) -> u32 {
    let mut r = x;
    while parent[r as usize] != r {
        r = parent[r as usize];
    }
    let mut c = x;
    while parent[c as usize] != r {
        let next = parent[c as usize];
        parent[c as usize] = r;
        c = next;
    }
    r
}

/// Number of connected components (union-find over welded vertices joined by
/// triangle edges). An empty mesh has zero components.
fn components(verts: &[[f64; 3]], tris: &[[u32; 3]]) -> usize {
    if tris.is_empty() {
        return 0;
    }
    let mut parent: Vec<u32> = (0..verts.len() as u32).collect();
    let mut used = vec![false; verts.len()];
    for t in tris {
        for &v in t {
            used[v as usize] = true;
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2])] {
            let ra = uf_find(&mut parent, a);
            let rb = uf_find(&mut parent, b);
            if ra != rb {
                parent[ra as usize] = rb;
            }
        }
    }
    let mut roots = std::collections::HashSet::new();
    for i in 0..verts.len() as u32 {
        if used[i as usize] {
            roots.insert(uf_find(&mut parent, i));
        }
    }
    roots.len()
}

/// Solid centroid via the tetrahedron-fan integral (same cross-product sum as
/// `signed_volume`). `None` for a zero-volume/empty mesh.
fn centroid(mesh: &Mesh) -> Option<[f64; 3]> {
    let mut vol = 0.0;
    let mut c = [0.0; 3];
    for t in &mesh.tris {
        let a = mesh.verts[t[0] as usize];
        let b = mesh.verts[t[1] as usize];
        let d = mesh.verts[t[2] as usize];
        let cr = [
            b[1] * d[2] - b[2] * d[1],
            b[2] * d[0] - b[0] * d[2],
            b[0] * d[1] - b[1] * d[0],
        ];
        let vt = (a[0] * cr[0] + a[1] * cr[1] + a[2] * cr[2]) / 6.0;
        vol += vt;
        for i in 0..3 {
            c[i] += vt * (a[i] + b[i] + d[i]) / 4.0;
        }
    }
    if vol.abs() < 1e-9 {
        return None;
    }
    Some([c[0] / vol, c[1] / vol, c[2] / vol])
}

fn metrics_to_golden(m: &GeomMetrics) -> String {
    let mut s = String::new();
    s.push_str(&format!("volume {:.6}\n", m.volume));
    if let Some((lo, hi)) = m.bbox {
        s.push_str(&format!(
            "bbox {:.6} {:.6} {:.6} {:.6} {:.6} {:.6}\n",
            lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
        ));
    }
    if let Some(c) = m.centroid {
        s.push_str(&format!("centroid {:.6} {:.6} {:.6}\n", c[0], c[1], c[2]));
    }
    s.push_str(&format!("components {}\n", m.components));
    s.push_str(&format!("manifold {}\n", m.manifold));
    s.push_str(&format!("tris {}\n", m.tris));
    s
}

struct Golden {
    volume: f64,
    bbox: Option<[f64; 6]>,
    centroid: Option<[f64; 3]>,
    components: usize,
    manifold: bool,
    tris: usize,
}

fn parse_golden(text: &str) -> Option<Golden> {
    let mut volume = None;
    let mut bbox = None;
    let mut centroid = None;
    let mut components = None;
    let mut manifold = None;
    let mut tris = None;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let nums = |it: std::str::SplitWhitespace| -> Vec<f64> {
            it.filter_map(|s| s.parse().ok()).collect()
        };
        match it.next()? {
            "volume" => volume = it.next()?.parse().ok(),
            "bbox" => {
                let v = nums(it);
                if v.len() == 6 {
                    bbox = Some([v[0], v[1], v[2], v[3], v[4], v[5]]);
                }
            }
            "centroid" => {
                let v = nums(it);
                if v.len() == 3 {
                    centroid = Some([v[0], v[1], v[2]]);
                }
            }
            "components" => components = it.next()?.parse().ok(),
            "manifold" => manifold = Some(it.next()? == "true"),
            "tris" => tris = it.next()?.parse().ok(),
            _ => {}
        }
    }
    Some(Golden {
        volume: volume?,
        bbox,
        centroid,
        components: components?,
        manifold: manifold?,
        tris: tris?,
    })
}

/// Reasons the actual metrics diverge from the golden (empty = pass).
fn compare(g: &Golden, m: &GeomMetrics, d: &Directives) -> Vec<String> {
    let mut f = Vec::new();
    let vtol = (g.volume.abs() * d.vol_tol).max(VOL_ABS);
    if (g.volume - m.volume).abs() > vtol {
        f.push(format!("volume {:.6} vs {:.6} (tol {:.6})", m.volume, g.volume, vtol));
    }
    match (g.bbox, m.bbox) {
        (Some(gb), Some((lo, hi))) => {
            let got = [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]];
            for i in 0..6 {
                if (gb[i] - got[i]).abs() > d.bbox_tol {
                    f.push(format!("bbox[{i}] {:.4} vs {:.4} (tol {})", got[i], gb[i], d.bbox_tol));
                }
            }
        }
        (None, None) => {}
        (g, a) => f.push(format!("bbox present: golden {} actual {}", g.is_some(), a.is_some())),
    }
    match (g.centroid, m.centroid) {
        (Some(gc), Some(c)) => {
            for i in 0..3 {
                if (gc[i] - c[i]).abs() > d.bbox_tol {
                    f.push(format!("centroid[{i}] {:.4} vs {:.4} (tol {})", c[i], gc[i], d.bbox_tol));
                }
            }
        }
        (None, None) => {}
        (g, a) => f.push(format!("centroid present: golden {} actual {}", g.is_some(), a.is_some())),
    }
    if g.components != m.components {
        f.push(format!("components {} vs {}", m.components, g.components));
    }
    if g.manifold != m.manifold {
        f.push(format!("manifold {} vs {}", m.manifold, g.manifold));
    }
    if d.pin_tris && g.tris != m.tris {
        f.push(format!("tris {} vs {}", m.tris, g.tris));
    }
    f
}

/// Render a corpus case with quito's native pipeline (imports/`surface` resolve
/// relative to the case's directory, like OpenSCAD).
fn quito_mesh(case: &Path) -> Result<Mesh, String> {
    let src = fs::read_to_string(case).map_err(|e| e.to_string())?;
    let dir = case
        .parent()
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".into());
    let prog = quito_syntax::parse(&src).map_err(|e| format!("parse: {}", e.message))?;
    let out = quito_eval::eval_program_with(&prog, &DiskResolver, &dir).map_err(|e| format!("eval: {}", e.0))?;
    quito_geom::render(&out.node).map_err(|e| format!("render: {e}"))
}

fn bless_geom(cases: &Path, goldens: &Path) {
    fs::create_dir_all(goldens).unwrap();
    let tmp = std::env::temp_dir().join("quito_geom_bless.stl");
    let mut n = 0;
    for case in scad_cases(cases) {
        let _ = fs::remove_file(&tmp);
        let out = Command::new("openscad")
            .arg("-o")
            .arg(&tmp)
            .args(["--export-format", "binstl"])
            .arg(&case)
            .output()
            .expect("failed to run openscad — is it installed and on PATH?");
        // OpenSCAD writes no file (and exits nonzero) for an empty top-level
        // object — that is a valid "renders to nothing" case (empty mesh).
        let mesh = match fs::read(&tmp) {
            Ok(bytes) if !bytes.is_empty() => Mesh::from_stl(&bytes),
            _ => Mesh::new(),
        };
        let name = case.file_stem().unwrap().to_string_lossy();
        if !out.status.success() && !mesh.tris.is_empty() {
            eprintln!("  !  {name}: openscad exited nonzero but wrote geometry");
        }
        fs::write(goldens.join(format!("{name}.txt")), metrics_to_golden(&metrics(&mesh))).unwrap();
        n += 1;
    }
    eprintln!("blessed {n} geom goldens into {}", goldens.display());
}

fn check_geom(cases: &Path, goldens: &Path) -> bool {
    let case_list = if cases.is_dir() { scad_cases(cases) } else { Vec::new() };
    if case_list.is_empty() {
        eprintln!("geom oracle: no cases in {} — corpus missing or empty", cases.display());
        return false;
    }
    let mut pass = 0;
    let mut total = 0;
    let mut failures: Vec<(String, Vec<String>)> = Vec::new();

    for case in case_list {
        let name = case.file_stem().unwrap().to_string_lossy().to_string();
        total += 1;
        let Ok(golden_txt) = fs::read_to_string(goldens.join(format!("{name}.txt"))) else {
            failures.push((name, vec!["no golden (run `xtask bless-geom`)".into()]));
            continue;
        };
        let Some(golden) = parse_golden(&golden_txt) else {
            failures.push((name, vec!["malformed golden".into()]));
            continue;
        };
        let src = fs::read_to_string(&case).unwrap_or_default();
        let directives = parse_directives(&src);
        match quito_mesh(&case) {
            Ok(mesh) => {
                let reasons = compare(&golden, &metrics(&mesh), &directives);
                if reasons.is_empty() {
                    pass += 1;
                } else {
                    failures.push((name, reasons));
                }
            }
            Err(e) => failures.push((name, vec![e])),
        }
    }

    for (name, reasons) in &failures {
        println!("FAIL {name}");
        for r in reasons {
            println!("   - {r}");
        }
    }

    let pct = if total == 0 { 0.0 } else { pass as f64 / total as f64 * 100.0 };
    println!("\ngeom oracle: {pass}/{total} passed ({pct:.0}%)");
    pass == total && total > 0
}
