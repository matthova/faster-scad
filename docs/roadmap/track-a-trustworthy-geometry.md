# Track A — M6 "Trustworthy Geometry"

**Goal:** no known silent-wrong-geometry bug, and every geometry builtin
regression-guarded by a differential oracle against OpenSCAD 2024.12 that runs
in CI on every PR.

**Why now:** the project's methodology has been oracle-driven from the start
(echo goldens, dual-baseline perf tables), but geometry — the actual product —
is the one surface with no oracle. The bugs below are exactly the kind that
burn early adopters, because they don't error: they quietly produce a
different shape.

**Status:**
- **A1** (transform argument binding + axis-angle `rotate`) is **done** —
  named/positional/mixed args now bind on all four transforms and `rotate(a, v)`
  lowers to `MultMatrix` via Rodrigues; covered by eval- and geometry-level
  regression tests.
- **A2** (geometry oracle harness) is **done** — `xtask bless-geom` / `xtask geom`
  diff quito's native render against a 60-case corpus blessed from OpenSCAD 2024.12
  (volume, bbox, signed centroid, connected-component count, watertight+2-manifold,
  and opt-in triangle count), enforced in CI. A4/A5 add their cases to this
  corpus as they land.
- **A3** (clip `offset()` self-intersections) is **done** — `offset` now assembles
  the result from convex pieces (an offset slab per edge + a join cap per corner)
  unioned/subtracted through the `geo` clipper, so a concave inset larger than a
  local feature collapses to empty instead of filling a self-intersecting bowtie.
  Covered by new `corpus/geom` cases (concave inset/collapse/grow, hole-close) and
  `shape2d` unit tests. (Note: the doc's original "self-union" suggestion does not
  work in geo 0.29, which has no `unary_union` and does not resolve a
  self-intersecting input ring — hence the convex-piece construction.)
- **A4** (fix `projection()` dropped inside 2D booleans) is **done** — a projection
  reaching the pure-2D `render2d` (under `offset`/`hull`/`minkowski`, or a bare 2D
  boolean) used to fall through to empty geometry, silently. `render2d` has no
  kernel, so it can't render the projection's 3D child; instead a `Ctx`-aware
  pre-pass (`lower_projections`) resolves every projection to a `Polygon` leaf
  (via `slice_z0`/`silhouette`) before `render2d` runs. Covered by `corpus/geom`
  cases (projection under offset/hull, both `cut` modes) and a unit test. (The
  doc's original "add a `render2d` arm" idea can't work as written — no kernel
  there. CLI 2D→DXF/SVG export of a bare projection stays unresolved, a separate
  niche gap.)

The remaining items (A5–A6) are still open.

**Effort:** ~1 week remaining (A5–A6). **Exit criterion** at the bottom.

---

## A5. `minkowski()`: stop being silently wrong for non-convex operands

### The bug

`minkowski_fold`/`minkowski_pair` (`crates/quito-geom/src/lib.rs:403-426`)
compute the convex hull of pairwise vertex sums — exact only when **both**
operands are convex. The doc comment admits it ("for non-convex operands it is
the convex Minkowski approximation"). The 2D path (`shape2d::minkowski_2d`,
lines 278–297) has the same limitation. The classic use case — rounding a
non-convex outline (L-bracket, gear) with a sphere/circle — silently returns
the convex hull: dramatically wrong, no warning.

### Two-stage plan

**Stage 1 (in M6, ~1 day): make it loud.** Detect non-convex operands (compare
operand volume/area against its own convex hull's within epsilon — both
quantities are already computable with existing code) and emit a rendering
warning through the existing console/warning channel: "minkowski: non-convex
operand; result is the convex approximation". Honesty beats silence; document
it in COMPAT.md's divergence register with this exact repro.

**Stage 2 (M6 stretch or M7, ~2+ weeks): make it right.** Exact non-convex
Minkowski via convex decomposition: decompose each operand into convex pieces,
Minkowski-sum each pair (the existing convex pair code is exactly right for
this), union all pairwise results (the kernel's job). Needs a convex
decomposition on both kernels — approximate convex decomposition is acceptable
if the union is taken afterward with slightly inflated pieces. 2D first
(ear-clip triangles as the decomposition, trivially available from the
existing earcut) — that alone fixes the most common case (2D rounding via
`minkowski() { poly; circle(r); }`), then 3D. Gate with oracle cases either
way.

---

## A6. BOSL2 suite: real assertions, running in CI

### What's wrong today (three compounding failures)

1. `.github/workflows/ci.yml` checks out without `submodules: true` and has no
   `xtask bosl2` step — the M2 exit metric has never run in CI.
2. `xtask/src/main.rs` `run_bosl2` (~lines 43–75) silently `continue`s on
   unreadable files (`let Ok(raw) = fs::read_to_string(&path) else
   { continue }`), so with the submodule absent it reports **0/0 and exits
   zero**. The denominator can shrink invisibly (README says 15/15; the code
   lists 16 names).
3. "Pass" means "eval returned Ok" — but BOSL2 tests verify via internal
   `assert()`s. A regression that makes an assert *not fire* (or makes the
   test vacuous) still passes.

### The fix

- ci.yml: `submodules: true` on checkout, add
  `cargo run -p xtask -- bosl2` as a step.
- run_bosl2: missing file or 0 executed tests → **hard failure** (nonzero
  exit, like `xtask echo`). Print per-file pass/fail.
- Assertion checking: capture eval diagnostics per test file; a failed
  `assert()` (which surfaces as an eval error or a warning-channel entry) →
  test fails. Where feasible, cross-check against
  `openscad --check`-style ground truth once during blessing, so we know each
  test file actually executes its assertions under quito (guards against
  vacuous passes where a comprehension short-circuits).
- Stretch: expand past the 16-name function subset toward BOSL2's geometry
  suites (attachments/shapes/transforms/vnf/rounding) — realistic only after
  A1 lands, and a good live-fire measure of how much A1 unlocks.

**Effort: ~1–2 days for the CI/hard-fail/assertion work; the suite expansion
is open-ended and can trail.**

---

## Also in scope if time allows

- **Assignment-hoisting pre-pass** (COMPAT.md divergence #2, still open):
  `eval_defs_and_assigns` (`quito-eval/src/lib.rs:341-386`) evaluates
  assignments in source order, so forward references (`y = x; x = 5;`) yield
  `undef` instead of upstream's "last assignment wins at first position".
  Language-level, oracle-checkable via echo corpus — a good companion fix
  while the evaluator is open, with a warn-on-reassignment lint per the plan's
  "warn-and-keep" candidate.
- **`rotate_extrude(start=)` and `linear_extrude(v=)`** parameters — small
  parser+IR additions, oracle-checkable, common in newer scripts.

## Exit criterion (crisp, CI-enforced)

> `cargo run -p xtask -- geom` passes **60/60** golden geometry cases
> (volume ±0.1%, bbox ±0.01 mm, manifoldness, component count; tessellation
> counts exact where pinned) blessed against OpenSCAD 2024.12, **and**
> `cargo run -p xtask -- bosl2` reports **16/16 with assertion-output
> checking**, both enforced in CI on every PR (0/0, skipped files, or missing
> corpus = red). The geom harness + corpus are already in place and green in CI
> (A2); named-arg/axis-angle transforms (A1), offset self-intersection (A3), and
> projection-in-2D-boolean (A4) are closed. Still open: non-convex minkowski at
> minimum emitting a user-visible warning (A5), and BOSL2 assertion-output
> checking (A6). COMPAT.md accurately lists every remaining known divergence.
