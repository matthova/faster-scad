---
"quito-release-root": patch
---

Web UI (M9): the **Custom** render-quality preset now exposes `$fa` (max fragment angle) and `$fs` (max fragment size) inputs alongside `$fn`. These were already persisted, validated, and injected into renders but had no UI; the tolerance knobs are how you match OpenSCAD's 12°/2 mm defaults on curves that don't set `$fn`.
