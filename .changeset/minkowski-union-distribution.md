---
"openrscad-release-root": patch
---

3D `minkowski()` now distributes over `union()`, so a concave shape built from a union of convex parts is computed exactly (e.g. `minkowski(){ union(){ cube A; cube B; } cube; }`) instead of as its convex hull. A genuinely concave leaf mesh still falls back to the convex approximation with a warning.
