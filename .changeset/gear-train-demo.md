---
"quito-release-root": patch
---

Add an animated BOSL2 gear-train demo, and fix two engine bugs it exposed: (1) an omitted function parameter now correctly shadows a same-named global (as `undef`) even inside `assert(...) expr` guard bodies, so BOSL2 gears.scad idioms like `circular_pitch()` no longer trip a spurious assertion; (2) `linear_extrude` now drops consecutive duplicate vertices, so profiles that emit zero-length edges (e.g. BOSL2's `rack2d`) produce manifold solids instead of degrading to un-combined geometry under boolean union.
