# Track B — "Switcher Experience" (proposed M7)

**Goal:** an OpenSCAD user can adopt OpenRSCAD (desktop or CLI) for a real project
without hitting a workflow wall. Every item below is a confirmed gap where
OpenRSCAD breaks a daily habit an OpenSCAD user already has.

**Why after track A:** each of these polishes the adoption funnel. Funneling
users into an engine that still silently mis-rotates `rotate(45,[1,1,0])`
(track A, item A1) converts adopters into detractors — trust first, comfort
second. Sequenced as M7, it inherits a geometry engine users can rely on.

**Status: complete.** All of B1–B5 have shipped; this track is closed.

> **Done:** B1 (desktop Save source — `save_source`/`watch_files` commands, ⌘S /
> Save As, native menu, dirty-tab indicator, watcher reentrancy, `.scad` file
> association) and B2 (inline editor diagnostics — statement spans threaded
> through the AST/evaluator, a structured `diagnostics` JSON channel across the
> wasm + Tauri boundaries, and `@codemirror/lint` squiggles: red for errors,
> yellow for warnings, cleared on success, with a badge on the main tab when it
> isn't focused), and B3 (`color()`/`#`/`%` in the preview — color/highlight/
> background IR nodes, a preview-only color-group partition rendered as per-group
> three.js materials with a fused mesh kept for stats/export/oracle, and
> per-object color 3MF export), and B4 (PNG export — a Save-PNG button that
> captures the three.js viewer in the playground and desktop app, plus a headless
> pure-Rust CLI rasterizer, `openrscad … -o out.png` with OpenSCAD-style
> `--imgsize`/`--camera`/`--projection`/`--viewall`/`--autocenter`, colored via the
> B3 groups), and B5 (parity bundle: a `text(font=)` warning; customizer
> parameter sets — CLI `-p`/`-P` + web preset dropdown with save/load/import/export
> of OpenSCAD `.json`; animation export — CLI `--animate N` + a web frames→zip
> button; and `$vpr`/`$vpt`/`$vpd`/`$vpf` viewport variables — read + write, so
> scripts can react to and drive the camera) shipped. **Track B is complete.**

---

## Exit criterion — met

> A user opens a multi-file OpenSCAD project in OpenRSCAD desktop — ⌘S save, inline
> error squiggles, `color()`/`#`/`%` in the preview, STL + PNG export, customizer
> presets, animation frame dumps, and `$vp*`-driven cameras all work — without
> touching OpenSCAD. (COMPAT.md divergence #5 is closed.)
