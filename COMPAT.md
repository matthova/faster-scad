# Compatibility divergence register

Quito targets OpenSCAD 2021.01 semantics "in spirit." Intentional divergences
are recorded here with a repro. Bug-for-bug fidelity is a non-goal.

## Current M0 divergences / simplifications

These are known gaps in the M0 walking skeleton, to be closed in later
milestones (not permanent divergences unless noted):

| # | Area | M0 behavior | Upstream | Milestone to close |
|---|---|---|---|---|
| 1 | Scoping | Dynamic scope stack; variable/`$`-var lookup walks the call stack. | Lexical slots for ordinary vars; dynamic scope for `$` vars. | M2 |
| 2 | Assignment hoisting | Assignments in a scope are hoisted then evaluated in source order (last write wins). Handles the common `x=1; …; x=2;` case; does not yet resolve forward references the way a full pre-pass does. | Full "last assignment wins at first position" pre-pass with a lint. | M2 |
| 3 | `children()` / `$children` | Not implemented; a call emits a warning and yields no geometry. | Full module children support. | M2 |
| 4 | echo number formatting | Approximate (round-trip shortest / 6-figure). | Exact 5-significant-digit formatting. | M2 (echo oracle) |
| 5 | Modifiers `#` / `%` | Parsed and passed through (no visual distinction; geometry unchanged). | Highlight / transparent-background rendering. | M5 (viewer) |
| 6 | Geometry breadth | Only `cube`/`sphere`/`cylinder` + `translate`/`rotate`/`scale` + booleans. No `polygon`/`polyhedron`/extrudes/`hull`/`minkowski`/`offset` yet. | Full primitive/transform set. | M3 |

## Candidate intentional cleanups (from the plan, not yet decided)

- Warn-and-keep on last-assignment-wins (footgun surfaced, behavior kept).
- Saner reversed-range handling.
- No bug-for-bug reproduction of CGAL/Nef coincident-face degeneracies.
- Tolerance-level (not vertex-exact) mesh equivalence as the compat bar.

_When a divergence becomes permanent by design, move it to a "Permanent
divergences" section with rationale._
