# Track C — CI Hardening (make the quality claims enforceable)

**Goal:** every headline claim in the README — 24/24 echo oracle, BOSL2 suite,
~25× perf, working playground — is machine-enforced on every PR, and the parts
of the codebase that currently never build in CI (desktop, web PRs) do.

**Positioning:** lowest standalone user-visible value of the three tracks, but
several items are prerequisites or force-multipliers for track A (BOSL2 gating,
geometry-oracle CI step) and should be folded into whichever milestone runs
first rather than scheduled as their own. Items are ordered by
severity-of-blind-spot.

**Effort:** ~1.5–2 weeks total if done as one push; most items are
independent, small, and parallelizable.

---

## C1. BOSL2 suite in CI (shared with track A, item A6)

The single worst blind spot: an advertised exit metric that has never run in
CI. `ci.yml` checks out without `submodules: true`, has no
`cargo run -p xtask -- bosl2` step, and `run_bosl2` silently reports **0/0
with exit code 0** when `corpus/BOSL2` is absent (the `else { continue }` on
unreadable files shrinks the denominator invisibly — README says 15/15, the
code lists 16 names). Fix per A6: submodule checkout, the xtask step,
hard-fail on 0-executed, assertion-output checking. *Small.*

## C2. Web app typechecked and built on PRs

`deploy-pages.yml` runs `tsc`/`vite build` only on push to `main` — a PR can
merge TypeScript that breaks the live site, and the breakage is discovered by
the deploy. Add a CI job on PRs: `npm ci && npm run build:wasm && tsc -b &&
vite build` in `web/` (cache the wasm-pack artifacts; the wasm build is the
slow part and can reuse the existing cargo cache key). This also implicitly
smoke-tests that `quito-wasm` actually *builds with wasm-pack* rather than
just `cargo check`-ing for the target. *Small.*

## C3. Desktop builds in CI

`desktop/src-tauri` is outside the cargo workspace, so **no CI job compiles it
at all** — engine-surface refactors can break the desktop app silently (this
already required a manual `Cargo.lock` sync commit, `0906018`). Add a
`cargo build --manifest-path desktop/src-tauri/Cargo.toml` job (Linux runner
with the Tauri system deps, or macOS runner — macOS matches the actual target
platform and needs no apt setup). Full bundling isn't needed; compile is the
regression gate. *Small, plus one-time runner-deps fiddling.*

## C4. Lint gates: clippy + rustfmt + web lint

No `cargo clippy` or `cargo fmt --check` anywhere. Add
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo fmt --check`; fix the initial fallout once (expect a modest, mostly
mechanical batch). Web side has zero lint config — adding eslint is optional,
but `tsc` on PRs (C2) is the non-negotiable part. Also fix the stale
`ci.yml` push branch filter still referencing `matthova/quito-v1`
(cherry-pick list). *Small.*

## C5. Perf regression tracking in CI

`xtask bench` compares against OpenSCAD binaries, so it can't run in CI — but
the *absolute* numbers can. Two-tier approach:

1. **Warm-edit gate (the M4 promise):** a CI job that renders each
   `benches/models/*.scad` cold then warm in-process and asserts warm-edit
   < a generous threshold (e.g. 20 ms vs the <100 ms target — runner noise
   headroom). This guards the `GeomCache` against structural-hash or
   invalidation regressions, which are exactly the silent kind.
2. **Trend line (optional):** store cold-render times as a CI artifact /
   `gh-pages` JSON and alert on >2× jumps rather than hard-failing (shared
   runners are too noisy for tight absolute gates). `criterion` for
   micro-level (VM opcode, tessellation) benches is nice-to-have, local-only.

*Medium (~2–3 days).*

## C6. Fuzzing the parser and evaluator

The playground feeds arbitrary public input to the wasm engine, and there is
no fuzzing anywhere (no `fuzz/` dir, no cargo-fuzz). A worker panic in the
browser is survivable (worker restarts) but a reproducible hang/OOM is a
denial-of-usability, and the same crates run natively in the CLI/desktop.

- `cargo fuzz` targets: (1) `quito-syntax::parse` on arbitrary bytes —
  must never panic, only return errors; (2) parse-then-eval with a low
  `MAX_CALL_DEPTH` and an instruction/fuel budget — must never panic or
  loop forever (the fuel mechanism may need to be added to the evaluator,
  which is independently useful for the playground's cancellation story);
  (3) the mesh importers (`quito-geom/src/mesh.rs`, `vector2d.rs`) on
  arbitrary bytes — STL/OFF/OBJ/3MF/AMF/DXF/SVG parsers are classic
  fuzz-finds-crashes territory, and `import()` in the playground can fetch
  user-supplied files.
- Seed corpora exist for free: `corpus/echo/*.scad`, `benches/models/`,
  `examples/`, plus small binary fixtures from the A2 geometry corpus.
- CI: a short (5-minute) libFuzzer smoke job on PRs; long runs stay local or
  scheduled-nightly. Track crashers as regression tests.

*Medium (~2–3 days to stand up; ongoing value).*

## C7. Wasm tests actually executed

CI only *compile-checks* `quito-wasm` for `wasm32-unknown-unknown`; the 5
`#[test]`s in the crate run only on the host. Run them under
`wasm-pack test --headless --chrome` (or `wasm-bindgen-test-runner`) so the
boolmesh-kernel-on-wasm path — the one browsers actually execute, and the one
the Manifold-vs-boolmesh differential exists to protect — is tested in its
real environment. *Small.*

## C8. Web unit tests for the pure-logic modules

Zero JS/TS tests today. Highest-value, lowest-cost targets are the pure
functions: `web/src/stl.ts` (and the other export writers — byte-exact golden
outputs for tiny meshes), `customizer.ts` schema parsing, `share.ts`
lz-string round-trip. Vitest, no DOM needed. Not the viewer/App — those need
the E2E Playwright setup that already exists for manual verification and can
be promoted to CI separately if flake allows. *Small-medium.*

---

## Suggested execution

Fold **C1** into track A (it's literally A6). Do **C2+C3+C4** as one
"CI truthfulness" PR alongside the cherry-picks — they're each ~an hour of
YAML plus one-time fallout fixes. **C5.1** (warm-edit gate) rides with track
A's xtask work. **C6–C8** are independent and make good gap-filler tasks
between track A items.

## Exit criterion

> Every claim in the README is backed by a green CI job that would turn red if
> it stopped being true: echo 24/24, BOSL2 16/16, geometry corpus (once track
> A lands), warm-edit <100 ms, web builds on PRs, desktop compiles, clippy/fmt
> clean, and a fuzz smoke-run with zero known crashers.
