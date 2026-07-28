#![no_main]
//! Parse-then-eval must never panic, hang, or OOM on arbitrary input — the
//! playground evaluates untrusted public source, and the same evaluator runs
//! natively in the CLI/desktop.
//!
//! Two things make this a *clean* fuzz target rather than a hang generator:
//!  - a fuel budget, so adversarial programs (e.g. nested `for` loops whose
//!    product dwarfs the per-construct limits) always terminate with an error;
//!  - a large stack matching the real CLI/wasm deployments (the CLI uses 256
//!    MiB), so a deeply-nested AST doesn't report a stack overflow that no real
//!    deployment would hit — libFuzzer's default main-thread stack is far
//!    smaller than any of them.
use libfuzzer_sys::fuzz_target;

const STACK: usize = 256 << 20;
// Generous for real programs; small enough to cut a runaway off in milliseconds.
const BUDGET: u64 = 5_000_000;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(prog) = quito_syntax::parse(src) else {
        return;
    };
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || {
            // No file access: `include`/`use` degrade to warnings.
            let _ = quito_eval::eval_program_with_budget(&prog, &quito_eval::NullResolver, ".", BUDGET);
        })
        .unwrap()
        .join()
        .expect("eval must not panic");
});
