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
| 5 | Modifiers `#` / `%` | Parsed and passed through (no visual distinction; geometry unchanged). | Highlight / transparent-background rendering. | Track B (item B3) |

## Current known divergences (with repro)

These produce output that differs from OpenSCAD — several **silently**, which is
the dangerous kind. Each has a minimal repro. Tracks A/B track the fixes.

- **Named-argument transforms are unbound.** `translate`/`rotate`/`scale`/
  `mirror` read only their first *positional* argument, so the named form binds
  nothing and falls back to identity — the transform silently does nothing.

  ```scad
  translate(v = [10, 0, 0]) cube(1);   // not moved (should shift +10 X)
  ```

- **Axis-angle `rotate(a, v)` ignores the axis.** Only `rotate(scalar)` (about
  Z) and `rotate([x,y,z])` (Euler) are handled; the two-argument axis-angle form
  keeps the angle but drops the axis vector, rotating about Z instead.

  ```scad
  rotate(45, [1, 1, 0]) cube(10);      // rotates about Z, not the [1,1,0] axis
  ```

- **Non-convex `minkowski()` is a convex approximation.** The Minkowski sum is
  computed as though both operands were convex, so a concave input gives the
  wrong swept solid.

  ```scad
  minkowski() { difference() { square(20); square([20,10]); } circle(2); }
  ```

- **Concave `offset()` does not clip self-intersections.** An inward/outward
  offset that would make an edge cross itself is left self-intersecting instead
  of being clipped.

  ```scad
  offset(r = -3) polygon([[0,0],[10,0],[10,2],[2,2],[2,10],[0,10]]);
  ```

- **`projection()` is dropped inside 2D booleans.** A `projection()` used as an
  operand of a 2D `union`/`difference`/`intersection` contributes nothing.

  ```scad
  difference() { square(10); projection() translate([0,0,-1]) sphere(4); }
  ```

- **`color()` is not rendered.** Children pass through and render, but the color
  is discarded (geometry is always the default material). No visible color in
  the viewer or exports.

  ```scad
  color("red") cube(10);               // renders, but not red
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
