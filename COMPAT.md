# Compatibility divergence register

OpenRSCAD targets OpenSCAD 2021.01 semantics "in spirit." Intentional divergences
and known gaps are recorded here with a repro. Bug-for-bug fidelity is a
non-goal, but a *silently wrong answer* is a trust bug — every one we know about
is listed below so a switcher hits a documented limitation, never an
undocumented wrong result.

## Open language gaps

Known gaps in the interpreter, to be closed in later milestones (not permanent
divergences unless noted):

_None currently open — assignment hoisting (formerly gap #2) was closed; see
below._

## Current known divergences (with repro)

These produce output that differs from OpenSCAD — several **silently**, which is
the dangerous kind. Each has a minimal repro. Tracks A/B track the fixes.

- **3D `minkowski()` is exact for convex operands and unions of them; a concave
  *leaf* mesh is a convex approximation (warned).** The convex-convex sum is the
  convex hull of pairwise vertex sums. Minkowski distributes over `union()`
  (`(A₁∪A₂) ⊕ B = (A₁⊕B) ∪ (A₂⊕B)`), so a non-convex shape *built from a union of
  convex parts* — the common way to build concave shapes — is now **exact**
  (`corpus/geom/minkowski_union.scad`). A genuinely concave *leaf* (e.g. a
  concave polygon extruded, or a non-convex polyhedron) still falls back to its
  convex hull with a warning — exact convex decomposition of arbitrary meshes is
  out of scope (it is research-grade and, as in OpenSCAD's CGAL, impractically
  slow). **2D `minkowski()` is always exact** (each operand is triangulated and
  the pairwise sums are unioned).

  ```scad
  // exact: concave shape as a union of convex parts
  minkowski() { union() { cube([10,4,4]); cube([4,10,4]); } cube(2, center=true); }
  // approximated + warned: a concave leaf that can't be peeled into a union
  minkowski() { linear_extrude(6) polygon([[0,0],[24,0],[24,6],[6,6],[6,24],[0,24]]); sphere(2); }
  ```

- **`text(font=…)` supports the bundled Liberation family only.** The full
  Liberation family — Sans / Serif / Mono × Regular / Bold / Italic / Bold Italic,
  the exact Liberation 2.00.1 files OpenSCAD ships — is bundled, so
  `font="Liberation Serif"`, `font="Liberation Sans:style=Bold"`, etc. select the
  same face and render byte-for-byte like OpenSCAD (verified by
  `corpus/geom/text_*`). A request for any *other* family (a system font, which is
  non-portable and would not exist in the browser) falls back to Liberation Sans
  **with a warning**, rather than silently. Arbitrary system-font resolution is
  intentionally out of scope: it is non-portable and non-reproducible against the
  oracle.

  ```scad
  text("Ag", font = "Liberation Serif:style=Bold");  // exact match
  text("Ag", font = "Courier New");                  // warns, falls back to Liberation Sans
  ```

- **`rands()` is not bit-compatible (documented).** OpenRSCAD uses an xorshift PRNG;
  values are reproducible and the global/seeded advance semantics match
  OpenSCAD, but the numbers do not match OpenSCAD's generator bit-for-bit. This
  divergence is intentional and permanent.

  ```scad
  echo(rands(0, 1, 3, seed = 42));     // reproducible, but ≠ OpenSCAD's values
  ```

- **BOSL2 function-suite coverage is partial (recorded, gated).** `cargo run -p
  xtask -- bosl2` runs **every** `[[test]]` block of the 15 function-oriented
  files and currently passes **503/513**; the gate now also honors each block's
  `expect_success` flag, so error-path tests pass when OpenRSCAD correctly *rejects*
  bad input. The ~10 remaining gaps are scattered edge cases (e.g. `rands`
  distribution — an intentional, documented divergence; inverse-trig exactness at
  nice angles; a few string/struct helpers). The passing set is pinned per file
  under `corpus/golden/bosl2/`, so these gaps are recorded and a regression fails
  CI — they are not a silent loss. Closing them is open-ended and tracked
  separately.

## Closed since M0

All oracle-checked; `corpus/echo` passes **25/25** and BOSL2's function suite
runs in the `xtask bosl2` harness:

- **Assignment hoisting (last-write-wins).** Within a scope, only a variable's
  *final* assignment is evaluated, at the point it was *first introduced*. A read
  of a variable that is reassigned later sees the final value, not the
  intermediate one (`p = 1; q = p; p = 5;` → `q == 5`), and an overwritten
  assignment's RHS is discarded entirely, side effects included. There are **no
  forward references**: a read of a variable introduced later in the scope does
  not see it and falls through to an outer binding or `undef`
  (`y = x; x = 5;` → `y == undef` at top level; the nested case reads the outer
  binding). Earlier docs described the target as full forward-reference
  resolution — the OpenSCAD oracle showed that is *not* how upstream behaves, so
  this now matches observed OpenSCAD 2024.12 (`corpus/echo/assign_hoist.scad`).
  A reassigned name emits a spanned "assigned again … overwritten" lint on the
  dead write (deduped per source site; suppressed for `include`d/`use`d library
  code). The companion "Ignoring unknown variable" lint is **deferred**: library
  helpers evaluate in the caller's statement context and legitimately read unset
  parameters as `undef`, so warning on unbound reads produced false positives on
  real BOSL2 code — closing it needs read-site provenance the AST does not yet
  carry.

- **Oblique / offset extrudes.** `linear_extrude(height=h, v=[x,y,z])` sweeps
  the profile a distance `h` along `normalize(v)` (an oblique prism); oracle-gated
  by `corpus/geom/ext_linear_v*.scad`. `rotate_extrude(angle=a, start=s)` sweeps
  from `s` to `s + a` about Z — equivalent to the `[0, a]` sweep rotated by `s`.
  The 2024.12 oracle predates `start=` (it warns "variable start not specified as
  parameter"), so `start=` is verified by rigid-motion invariant (volume
  preserved, geometry rotated) rather than an echo/geom golden.

- Geometry breadth — the full 2D/3D primitive, transform, extrude, `hull`,
  `minkowski`, and `offset` surface shipped in M3 (was M0's cube/sphere/cylinder
  + booleans only).
- Echo number formatting (`%.6g`, stripped exponents); list comprehensions
  (`for`/`if`/`let`/`each`, nested, and the C-style 2019.05
  `for(init;cond;update)` form); string quoting in `echo` + `str`/`chr`/`ord`/
  string-indexing; `search`/`lookup`/`is_*`; module `children()`/`$children`;
  function literals; **lexical scoping** for ordinary variables/functions/
  modules with dynamic scoping for `$` variables; and the `polyhedron`
  primitive.

## Candidate intentional cleanups (from the plan, not yet decided)

- Warn-and-keep on last-assignment-wins (footgun surfaced, behavior kept).
- Saner reversed-range handling.
- No bug-for-bug reproduction of CGAL/Nef coincident-face degeneracies.
- Tolerance-level (not vertex-exact) mesh equivalence as the compat bar.

_When a divergence becomes permanent by design, move it to a "Permanent
divergences" section with rationale._
