# Compatibility divergence register

Quito targets OpenSCAD 2021.01 semantics "in spirit." Intentional divergences
and known gaps are recorded here with a repro. Bug-for-bug fidelity is a
non-goal, but a *silently wrong answer* is a trust bug — every one we know about
is listed below so a switcher hits a documented limitation, never an
undocumented wrong result.

## Open language gaps

Known gaps in the interpreter, to be closed in later milestones (not permanent
divergences unless noted):

| # | Area | Current behavior | Upstream | Track to close |
|---|---|---|---|---|
| 2 | Assignment hoisting | Assignments in a scope are hoisted then evaluated in source order (last write wins). Handles the common `x=1; …; x=2;` case; does not yet resolve forward references the way a full pre-pass does. | Full "last assignment wins at first position" pre-pass with a lint. | Track A |

## Current known divergences (with repro)

These produce output that differs from OpenSCAD — several **silently**, which is
the dangerous kind. Each has a minimal repro. Tracks A/B track the fixes.

- **Non-convex 3D `minkowski()` is a convex approximation (now warned).** The 3D
  Minkowski sum is the convex hull of pairwise vertex sums — exact only for
  convex operands. A concave 3D operand yields the convex swept solid; the
  renderer now emits a warning (`minkowski: non-convex operand; result is the
  convex approximation`) rather than failing silently. **2D `minkowski()` is now
  exact** (each operand is triangulated and the pairwise sums are unioned), so
  the common `minkowski(){ poly; circle; }` rounding case is correct. Exact
  non-convex *3D* minkowski (convex decomposition) is deferred to a later
  milestone.

  ```scad
  minkowski() { linear_extrude(6) polygon([[0,0],[24,0],[24,6],[6,6],[6,24],[0,24]]); sphere(2); }
  ```

- **`text(font=…)` is ignored.** Only the bundled Liberation Sans (the face
  OpenSCAD ships) is available; any `font=` request is silently substituted with
  it, so glyphs differ for other fonts.

  ```scad
  text("Ag", font = "Courier New");    // rendered in Liberation Sans
  ```

- **`rands()` is not bit-compatible (documented).** Quito uses an xorshift PRNG;
  values are reproducible and the global/seeded advance semantics match
  OpenSCAD, but the numbers do not match OpenSCAD's generator bit-for-bit. This
  divergence is intentional and permanent.

  ```scad
  echo(rands(0, 1, 3, seed = 42));     // reproducible, but ≠ OpenSCAD's values
  ```

## Closed since M0

All oracle-checked; `corpus/echo` passes **24/24** and BOSL2's function suite
runs in the `xtask bosl2` harness:

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
