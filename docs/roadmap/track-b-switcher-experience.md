# Track B — "Switcher Experience" (proposed M7)

**Goal:** an OpenSCAD user can adopt Quito (desktop or CLI) for a real project
without hitting a workflow wall. Every item below is a confirmed gap where
Quito breaks a daily habit an OpenSCAD user already has.

**Why after track A:** each of these polishes the adoption funnel. Funneling
users into an engine that still silently mis-rotates `rotate(45,[1,1,0])`
(track A, item A1) converts adopters into detractors — trust first, comfort
second. Sequenced as M7, it inherits a geometry engine users can rely on.

**Effort:** a few days remaining (B5 bundle).

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
> pure-Rust CLI rasterizer, `quito … -o out.png` with OpenSCAD-style
> `--imgsize`/`--camera`/`--projection`/`--viewall`/`--autocenter`, colored via the
> B3 groups) shipped and are removed from this doc.

---

## B5. Smaller parity items (bundle opportunistically)

- **Customizer parameter sets** — OpenSCAD's `.json` parameter-set files
  (`-p file.json -P setname` on the CLI, preset dropdown + save/load in the
  UI). The customizer schema and override plumbing already exist
  (`quito-syntax/src/customizer.rs`, `-D` handling); this is a
  serialization format + UI dropdown. *Small-medium.*
- **Animation export** — `$t` playback exists (`web/src/App.tsx:607-652`) but
  there's no frame dump. GUI: render N frames → zip of PNGs or an animated
  WebP/GIF client-side. CLI: `--animate N` writing frame PNGs (depends on B4's
  rasterizer). *Small once B4 lands.*
- **`text()` font parameter** — currently silently ignored (bundled Liberation
  Sans only, `crates/quito-eval/src/text.rs`). Minimum honest fix: warn when
  `font=` names anything else (pairs with track A's warning discipline).
  Real fix (system font lookup via `fontdb`, shaping) is larger and can wait;
  ttb/btt text directions likewise. *Warn: trivial. Full: large.*
- **`$vpr`/`$vpt`/`$vpd` viewport variables** — read side (scripts reacting to
  camera) requires viewer→engine plumbing; write side (scripts setting the
  camera) is easy and is what animations typically use. *Small for write-side.*

## Exit criterion

> A user opens a multi-file OpenSCAD project in Quito desktop — ⌘S save, inline
> error squiggles, `color()`/`#`/`%` in the preview, and STL + PNG export all work
> now (B1–B4) — without touching OpenSCAD. (COMPAT.md divergence #5 is closed.)
> The remaining B5 bundle is opportunistic parity polish.
