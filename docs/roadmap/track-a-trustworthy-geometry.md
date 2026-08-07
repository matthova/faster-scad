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

- **A5** (`minkowski()` non-convex operands) is **done for 2D, loud for 3D** —
  2D minkowski is now **exact**: each operand is triangulated (earcut) and the
  pairwise triangle sums are unioned, so rounding a non-convex outline
  (`minkowski(){ poly; circle }`) is correct (was the convex hull). 3D non-convex
  minkowski stays the convex approximation (exact 3D via convex decomposition is
  ~2 weeks, deferred to M7), but the renderer now emits a warning through a new
  geometry-warning channel (surfaced in CLI/playground/desktop) instead of
  silently misleading. Covered by a 2D oracle case, `shape2d` area tests, and a
  `render_cached_warns` warning test; recorded in COMPAT.md.
- **A6** (BOSL2 suite: real assertions in CI) is **done** — the CI wiring +
  hard-fail-on-missing + `asserts_run > 0` landed with Track C/C1; A6 closed the
  core gap: the gate ran only the *first* `[[test]]` block per file (a fake
  "15/15" over ~15 of 513 blocks). `run_bosl2` now runs **every** block of all 15
  files (**503/513** pass, up from 422 once function-literal `str()` serialization,
  the `is_num(nan)`/exact-`tan` fixes, and `expect_success`-aware error tests
  landed), a block counting as passing only if it evals and runs ≥1 assert (or, for
  an `expect_success = false` block, correctly errors). The passing set is pinned
  per file under `corpus/golden/bosl2/` (`xtask bless-bosl2`), and any regression,
  unblessed improvement, missing file, or zero-block run hard-fails CI. The ~10
  remaining known-failing blocks are recorded (COMPAT.md), not fixed — closing
  them is open-ended and trails.

**Track A (M6) is complete: A1–A6 all landed.**

---

## Also in scope if time allows

- **Assignment hoisting (COMPAT.md divergence #2) — done.** Phase-2 assignment
  evaluation now applies OpenSCAD's last-write-wins semantics: only a variable's
  final assignment is evaluated, at its first-introduction point. Blessing the
  behavior against the echo oracle disproved this note's premise — upstream does
  **not** resolve forward references (`y = x; x = 5;` yields `undef`, not `5`);
  the real divergence was that reassign-then-earlier-read read the intermediate
  value instead of the final one (`p = 1; q = p; p = 5;` gave `q == 1`, now
  `q == 5`). Covered by `corpus/echo/assign_hoist.scad` and eval unit tests. The
  optional warn-on-reassignment lint ("warn-and-keep") was **added** for the
  reassignment case only: a dead write emits a spanned, deduped "overwritten"
  warning, restricted to the user's own source. The "unknown variable" companion
  lint was attempted and **deferred** — it false-positived on real BOSL2 code
  (library helpers read unset params as `undef` in the caller's statement
  context), and separating those from genuine user typos needs read-site
  provenance the AST does not carry.
- **`rotate_extrude(start=)` and `linear_extrude(v=)`** parameters — **done.**
  `v=` (an oblique/sheared extrude) was already implemented; it is now gated by
  `corpus/geom/ext_linear_v*.scad` oracle goldens. `start=` (partial-sweep offset)
  is new; it lowers to the plain `[0, angle]` sweep rotated by `start` about Z.
  The 2024.12 oracle predates `start=`, so it is verified by rigid-motion
  invariant (a render test) rather than a geom golden.

## Exit criterion (crisp, CI-enforced) — **MET**

> `cargo run -p xtask -- geom` passes **71/71** golden geometry cases
> (volume ±0.1%, bbox ±0.01 mm, signed centroid, manifoldness, component count;
> triangle count where pinned) blessed against OpenSCAD 2024.12, **and**
> `cargo run -p xtask -- bosl2` runs every BOSL2 `[[test]]` block with
> assertion-output checking (**503/513**, passing set pinned per file), both
> enforced in CI on every PR (regression, unblessed improvement, skipped/missing
> file, or 0/0 = red).

All of A1–A6 are closed: named-arg/axis-angle transforms (A1), the geometry
oracle (A2), offset self-intersection (A3), projection-in-2D-boolean (A4),
minkowski (A5: 2D exact, 3D warns), and the BOSL2 block-level assertion gate
(A6). COMPAT.md accurately lists every remaining known divergence (non-convex 3D
minkowski; BOSL2 unimplemented corners; `rands`/`text(font=)`).
