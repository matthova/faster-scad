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
        "bless-bosl2" => bless_bosl2(&root),
        "bosl2" => {
            if !run_bosl2(&root) {
                std::process::exit(1);
            }
        }
        "bench" => run_bench(&root),
        "warm-gate" => {
            if !run_warm_gate(&root) {
                std::process::exit(1);
            }
        }
        other => {
            eprintln!("unknown command: {other}");
            eprintln!(
                "usage: xtask [bless-echo|echo|bless-geom|geom|bless-bosl2|bosl2|bench|warm-gate]"
            );
            std::process::exit(2);
        }
    }
}

/// The BOSL2 function-oriented test files (BSD-licensed submodule). Each holds
/// many `[[test]]` blocks; the gate runs **all** of them (M2 originally ran only
/// the first per file). `test_quaternions` is intentionally absent — it does not
/// exist in the pinned submodule.
const BOSL2_SUBSET: [&str; 15] = [
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

/// One extracted `[[test]]` block: its name, script, and whether it is expected
/// to evaluate successfully. `expect_success = false` blocks assert that bad
/// input is *rejected* — they pass when evaluation errors, not when it succeeds.
struct Bosl2Block {
    name: String,
    script: String,
    expect_success: bool,
}

/// Extract every `[[test]]` block from a `.scadtest` file.
fn extract_tests(raw: &str) -> Vec<Bosl2Block> {
    let mut out = Vec::new();
    for chunk in raw.split("[[test]]").skip(1) {
        let name = chunk.find("name = \"").and_then(|i| {
            let rest = &chunk[i + "name = \"".len()..];
            rest.find('"').map(|j| rest[..j].to_string())
        });
        let script = chunk.find("script = '''").and_then(|i| {
            let rest = &chunk[i + "script = '''".len()..];
            rest.find("'''").map(|j| rest[..j].to_string())
        });
        // `expect_success = false` (before the script body) inverts the pass
        // condition; absence means the default `true`.
        let expect_success = !chunk
            .split_once("script = '''")
            .map(|(head, _)| head)
            .unwrap_or(chunk)
            .contains("expect_success = false");
        if let (Some(name), Some(script)) = (name, script) {
            out.push(Bosl2Block {
                name,
                script,
                expect_success,
            });
        }
    }
    out
}

/// Whether a single BOSL2 block passes. A normal block must parse, evaluate
/// without error, and run at least one assert (0 asserts is a vacuous pass). An
/// `expect_success = false` block instead must parse but *fail* to evaluate —
/// it exists to prove bad input is rejected.
fn bosl2_block_passes(b: &Bosl2Block, dir_str: &str) -> bool {
    match quito_syntax::parse(&b.script) {
        Ok(prog) => {
            let result = quito_eval::eval_program_with(&prog, &DiskResolver, dir_str);
            if b.expect_success {
                matches!(result, Ok(out) if out.asserts_run > 0)
            } else {
                result.is_err()
            }
        }
        Err(_) => false,
    }
}

/// Run every `[[test]]` block of one file through quito; return the sorted names
/// of the blocks that pass (parse + eval Ok + at least one assert executed) and
/// the total block count. `Err` = the file is missing/unreadable/corrupt.
fn bosl2_file_results(dir: &Path, name: &str) -> Result<(Vec<String>, usize), String> {
    let raw =
        fs::read_to_string(dir.join(format!("{name}.scadtest"))).map_err(|e| e.to_string())?;
    let tests = extract_tests(&raw);
    if tests.is_empty() {
        return Err("no [[test]] blocks (corrupt or wrong format)".into());
    }
    let dir_str = dir.to_string_lossy().into_owned();
    let mut passing: Vec<String> = tests
        .iter()
        .filter(|b| bosl2_block_passes(b, &dir_str))
        .map(|b| b.name.clone())
        .collect();
    passing.sort();
    passing.dedup();
    Ok((passing, tests.len()))
}

/// Regenerate the BOSL2 baselines (the set of passing block names per file) from
/// quito's current behavior. Dev-only; needs the submodule checked out.
fn bless_bosl2(root: &Path) {
    let dir = root.join("corpus/BOSL2/tests");
    let goldens = root.join("corpus/golden/bosl2");
    fs::create_dir_all(&goldens).unwrap();
    for name in BOSL2_SUBSET {
        match bosl2_file_results(&dir, name) {
            Ok((passing, total)) => {
                fs::write(
                    goldens.join(format!("{name}.txt")),
                    format!("{}\n", passing.join("\n")),
                )
                .unwrap();
                eprintln!("  {name}: {}/{} blessed", passing.len(), total);
            }
            Err(e) => eprintln!("  !  {name}: {e}"),
        }
    }
    eprintln!("blessed BOSL2 baselines into {}", goldens.display());
}

/// Run BOSL2's function suite through quito, block by block, gated against the
/// committed per-file baseline of passing block names. Returns `true` only if
/// every file exists, has blocks, and its current passing set exactly matches
/// its baseline — a **regression** (a baselined block now failing) and an
/// unblessed **improvement** (a block now passing) both fail, as does a missing
/// file/golden or a zero-block run (submodule absent → 0/0 is never a pass).
/// Known-failing blocks are recorded by their absence from the baseline, so the
/// gate guards them without pretending they pass.
fn run_bosl2(root: &Path) -> bool {
    use std::collections::BTreeSet;
    let dir = root.join("corpus/BOSL2/tests");
    let goldens = root.join("corpus/golden/bosl2");
    let mut ok = true;
    let mut total_pass = 0usize;
    let mut total_blocks = 0usize;

    for name in BOSL2_SUBSET {
        let (passing, total) = match bosl2_file_results(&dir, name) {
            Ok(r) => r,
            Err(e) => {
                println!("  MISSING {name} ({e})");
                ok = false;
                continue;
            }
        };
        total_pass += passing.len();
        total_blocks += total;

        let Ok(golden_txt) = fs::read_to_string(goldens.join(format!("{name}.txt"))) else {
            println!("  ?  {name}: no baseline (run `xtask bless-bosl2`)");
            ok = false;
            continue;
        };
        let golden: BTreeSet<&str> = golden_txt.lines().filter(|l| !l.is_empty()).collect();
        let current: BTreeSet<&str> = passing.iter().map(String::as_str).collect();
        let regressions: Vec<&&str> = golden.difference(&current).collect();
        let improvements: Vec<&&str> = current.difference(&golden).collect();

        if regressions.is_empty() && improvements.is_empty() {
            println!("  PASS {name}: {}/{}", passing.len(), total);
        } else {
            ok = false;
            println!(
                "  FAIL {name}: {}/{} (baseline {})",
                passing.len(),
                total,
                golden.len()
            );
            for r in &regressions {
                println!("     - regression: {r} was passing, now fails");
            }
            for i in &improvements {
                println!("     + improvement: {i} now passes — run `xtask bless-bosl2`");
            }
        }
    }

    let pct = if total_blocks == 0 {
        0.0
    } else {
        total_pass as f64 / total_blocks as f64 * 100.0
    };
    println!("\nBOSL2 test blocks: {total_pass}/{total_blocks} ({pct:.0}%)");
    if total_blocks == 0 {
        eprintln!("error: no BOSL2 blocks executed — is the corpus/BOSL2 submodule checked out?");
        ok = false;
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

/// Warm-edit performance gate (the M4 promise) — CI-safe: no OpenSCAD binary
/// and no prebuilt release `quito` needed, unlike `bench`. Renders each
/// `benches/models/*.scad` cold (fresh cache) then warm (same cache, a fresh
/// but structurally identical re-eval → all cache hits) in-process with the
/// native kernel, and asserts the warm re-render stays under a generous
/// threshold.
///
/// This is the guard for `GeomCache`'s structural-hash / invalidation logic,
/// which fails silently: if caching regresses, the warm render recomputes
/// geometry so warm ≈ cold, and a heavy model blows the threshold. An empty or
/// unreadable model corpus is a hard failure — a broken glob must not report a
/// vacuous pass in CI (same principle as the bosl2/geom oracles).
fn run_warm_gate(root: &Path) -> bool {
    // Generous vs the <100 ms M4 target: shared CI runners are noisy even
    // best-of-N, but a blown cache (warm ≈ cold on the heavy models) lands far
    // above this, so the headroom doesn't cost sensitivity.
    const WARM_GATE_MS: f64 = 20.0;
    const WARM_RUNS: usize = 5;

    let dir = root.join("benches/models");
    let mut models: Vec<PathBuf> = match fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("scad"))
            .collect(),
        Err(e) => {
            eprintln!("warm-gate: cannot read {}: {e}", dir.display());
            return false;
        }
    };
    models.sort();
    if models.is_empty() {
        eprintln!(
            "warm-gate: no .scad models in {} — refusing to pass vacuously",
            dir.display()
        );
        return false;
    }

    println!(
        "Warm-edit gate — in-process, native kernel, best of {WARM_RUNS} runs (ms). \
         Threshold: warm < {WARM_GATE_MS:.0} ms\n"
    );
    println!("{:<14} {:>10} {:>10} {:>8}", "model", "cold", "warm", "");
    println!("{}", "-".repeat(44));

    let kernel = quito_geom::ManifoldKernel::new();
    let mut all_ok = true;
    for path in &models {
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
        let base_dir = path
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default();

        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("warm-gate: {name}: read failed: {e}");
                all_ok = false;
                continue;
            }
        };
        let prog = match quito_syntax::parse(&src) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("warm-gate: {name}: parse failed: {e}");
                all_ok = false;
                continue;
            }
        };
        let eval = || quito_eval::eval_program_with(&prog, &DiskResolver, &base_dir);
        let out = match eval() {
            Ok(o) => o,
            Err(e) => {
                eprintln!("warm-gate: {name}: eval failed: {}", e.message);
                all_ok = false;
                continue;
            }
        };

        let mut cache = quito_geom::GeomCache::new();
        let t0 = Instant::now();
        let _ = quito_geom::render_cached(&out.node, &kernel, &mut cache);
        let cold = t0.elapsed().as_secs_f64() * 1000.0;

        // Best-of-N warm: re-eval a fresh (structurally identical) tree each
        // iteration so the timing includes the cache lookup rather than reusing
        // a Mesh handle we happened to keep alive.
        let mut warm = f64::INFINITY;
        let mut render_failed = false;
        for _ in 0..WARM_RUNS {
            let out2 = match eval() {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("warm-gate: {name}: re-eval failed: {}", e.message);
                    render_failed = true;
                    break;
                }
            };
            let t1 = Instant::now();
            let _ = quito_geom::render_cached(&out2.node, &kernel, &mut cache);
            warm = warm.min(t1.elapsed().as_secs_f64() * 1000.0);
        }
        if render_failed {
            all_ok = false;
            continue;
        }

        let ok = warm < WARM_GATE_MS;
        all_ok &= ok;
        println!(
            "{:<14} {:>10.1} {:>10.2} {:>8}",
            name,
            cold,
            warm,
            if ok { "ok" } else { "FAIL" }
        );
    }

    if all_ok {
        println!("\nwarm-gate: PASS — every model warm-re-renders under {WARM_GATE_MS:.0} ms.");
    } else {
        eprintln!(
            "\nwarm-gate: FAIL — a model regressed (warm ≥ {WARM_GATE_MS:.0} ms) or failed to \
             render. A warm time near its cold time means the GeomCache stopped hitting."
        );
    }
    all_ok
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
            Err(e) => vec![format!("ERROR: {}", e.message)],
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
    let mut d = Directives {
        pin_tris: false,
        vol_tol: VOL_REL,
        bbox_tol: BBOX_ABS,
    };
    for line in src.lines() {
        let Some(rest) = line.trim().strip_prefix("// oracle:") else {
            continue;
        };
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
        f.push(format!(
            "volume {:.6} vs {:.6} (tol {:.6})",
            m.volume, g.volume, vtol
        ));
    }
    match (g.bbox, m.bbox) {
        (Some(gb), Some((lo, hi))) => {
            let got = [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]];
            for i in 0..6 {
                if (gb[i] - got[i]).abs() > d.bbox_tol {
                    f.push(format!(
                        "bbox[{i}] {:.4} vs {:.4} (tol {})",
                        got[i], gb[i], d.bbox_tol
                    ));
                }
            }
        }
        (None, None) => {}
        (g, a) => f.push(format!(
            "bbox present: golden {} actual {}",
            g.is_some(),
            a.is_some()
        )),
    }
    match (g.centroid, m.centroid) {
        (Some(gc), Some(c)) => {
            for i in 0..3 {
                if (gc[i] - c[i]).abs() > d.bbox_tol {
                    f.push(format!(
                        "centroid[{i}] {:.4} vs {:.4} (tol {})",
                        c[i], gc[i], d.bbox_tol
                    ));
                }
            }
        }
        (None, None) => {}
        (g, a) => f.push(format!(
            "centroid present: golden {} actual {}",
            g.is_some(),
            a.is_some()
        )),
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
    let out = quito_eval::eval_program_with(&prog, &DiskResolver, &dir)
        .map_err(|e| format!("eval: {}", e.message))?;
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
        fs::write(
            goldens.join(format!("{name}.txt")),
            metrics_to_golden(&metrics(&mesh)),
        )
        .unwrap();
        n += 1;
    }
    eprintln!("blessed {n} geom goldens into {}", goldens.display());
}

fn check_geom(cases: &Path, goldens: &Path) -> bool {
    let case_list = if cases.is_dir() {
        scad_cases(cases)
    } else {
        Vec::new()
    };
    if case_list.is_empty() {
        eprintln!(
            "geom oracle: no cases in {} — corpus missing or empty",
            cases.display()
        );
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

    let pct = if total == 0 {
        0.0
    } else {
        pass as f64 / total as f64 * 100.0
    };
    println!("\ngeom oracle: {pass}/{total} passed ({pct:.0}%)");
    pass == total && total > 0
}
