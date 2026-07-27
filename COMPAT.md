# Compatibility divergence register

Quito targets OpenSCAD 2021.01 semantics "in spirit." Intentional divergences
are recorded here with a repro. Bug-for-bug fidelity is a non-goal.

## Current M0 divergences / simplifications

These are known gaps in the M0 walking skeleton, to be closed in later
milestones (not permanent divergences unless noted):

| # | Area | Current behavior | Upstream | Milestone to close |
|---|---|---|---|---|
| 2 | Assignment hoisting | Assignments in a scope are hoisted then evaluated in source order (last write wins). Handles the common `x=1; …; x=2;` case; does not yet resolve forward references the way a full pre-pass does. | Full "last assignment wins at first position" pre-pass with a lint. | M2 |
| 5 | Modifiers `#` / `%` | Parsed and passed through (no visual distinction; geometry unchanged). | Highlight / transparent-background rendering. | M5 (viewer) |
| 6 | Geometry breadth | Only `cube`/`sphere`/`cylinder` + `translate`/`rotate`/`scale` + booleans. No `polygon`/`polyhedron`/extrudes/`hull`/`minkowski`/`offset` yet. | Full primitive/transform set. | M3 |

Closed since M0 (all oracle-checked, `corpus/echo` 16/16): echo number
formatting (`%.6g`, stripped exponents); list comprehensions (`for`/`if`/`let`/
`each`, nested, and the C-style 2019.05 `for(init;cond;update)` form); string
quoting in `echo` + `str`/`chr`/`ord`/string-indexing; `search`/`lookup`/`is_*`;
module `children()`/`$children`; function literals; **lexical scoping** for
ordinary variables/functions/modules with dynamic scoping for `$` variables;
and the `polyhedron` primitive.

## Permanent divergences (decided)

| Area | Quito | OpenSCAD | Rationale |
|---|---|---|---|
| Reversed ranges | `[5:0]` (or `[1:0]`) iterates empty; echoes as written, e.g. `[1 : 1 : 0]`. | Silently swaps bounds: `[1:0]` iterates `0,1` and echoes `[0 : 1 : 1]`. | The upstream swap is a well-known footgun. Quito treats a range whose direction contradicts its step as empty — predictable and matches the plan's "saner reversed-range handling." |

## Candidate intentional cleanups (from the plan, not yet decided)

- Warn-and-keep on last-assignment-wins (footgun surfaced, behavior kept).
- Saner reversed-range handling.
- No bug-for-bug reproduction of CGAL/Nef coincident-face degeneracies.
- Tolerance-level (not vertex-exact) mesh equivalence as the compat bar.

_When a divergence becomes permanent by design, move it to a "Permanent
divergences" section with rationale._
