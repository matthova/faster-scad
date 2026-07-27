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
        other => {
            eprintln!("unknown command: {other}");
            eprintln!("usage: xtask [bless-echo|echo]");
            std::process::exit(2);
        }
    }
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

    let pct = if total == 0 { 0.0 } else { pass as f64 / total as f64 * 100.0 };
    println!("\necho oracle: {pass}/{total} passed ({pct:.0}%)");
    pass == total
}
