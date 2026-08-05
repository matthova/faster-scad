---
"quito-release-root": minor
---

Web UI (M9 Phase 2 — the signature): **isolate and dimension any part of your model without touching the code.** The dock gains an **Objects** section listing the parts your script produced (with triangle counts); click one — or click a face in the viewport — and every other part is hidden, the ISO dimension callouts retarget to that part's bounding box, and the editor jumps to its source. The readout reports the isolated subset's triangles and extent (never volume — a subset of leaves isn't a closed solid). Escape (or "Show all") restores the whole model. Selection is viewport-only and non-destructive: no re-render, no edit to your file, and because it's world-space geometry it travels into PNG captures — a detail drawing of a sub-assembly, straight from code. The OpenSCAD engine has no provenance, so the section explains that instead of vanishing.
