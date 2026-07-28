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
        "bosl2" => {
            if !run_bosl2(&root) {
                std::process::exit(1);
            }
        }
        "bench" => run_bench(&root),
        other => {
            eprintln!("unknown command: {other}");
            eprintln!("usage: xtask [bless-echo|echo]");
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
                Err(e) => Err(format!("eval error: {}", e.message)),
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
