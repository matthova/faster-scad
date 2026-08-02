---
"quito-release-root": minor
---

add a "Fast" preview toggle that renders unions by concatenation instead of the CSG kernel — much faster on union-heavy models (skips the kernel's costliest work), at the cost of a non-watertight on-screen mesh. Differences, intersections and hulls still resolve exactly, so holes and clips look correct; exports and reported volume always use the exact, watertight path.
