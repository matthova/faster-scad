# Track B — "Switcher Experience" (proposed M7)

**Goal:** an OpenSCAD user can adopt Quito (desktop or CLI) for a real project
without hitting a workflow wall. Every item below is a confirmed gap where
Quito breaks a daily habit an OpenSCAD user already has.

**Why after track A:** each of these polishes the adoption funnel. Funneling
users into an engine that still silently mis-rotates `rotate(45,[1,1,0])`
(track A, item A1) converts adopters into detractors — trust first, comfort
second. Sequenced as M7, it inherits a geometry engine users can rely on.

**Effort:** ~1 week remaining.

> **Done:** B1 (desktop Save source — `save_source`/`watch_files` commands, ⌘S /
> Save As, native menu, dirty-tab indicator, watcher reentrancy, `.scad` file
> association) and B2 (inline editor diagnostics — statement spans threaded
> through the AST/evaluator, a structured `diagnostics` JSON channel across the
> wasm + Tauri boundaries, and `@codemirror/lint` squiggles: red for errors,
> yellow for warnings, cleared on success, with a badge on the main tab when it
> isn't focused), and B3 (`color()`/`#`/`%` in the preview — color/highlight/
> background IR nodes, a preview-only color-group partition rendered as per-group
> three.js materials with a fused mesh kept for stats/export/oracle, and
> per-object color 3MF export) shipped and are removed from this doc.

---

## B4. Image (PNG) export — GUI button, then CLI `--render`

### Current state

No raster output exists anywhere. `crates/quito-cli/src/main.rs` (~lines
188–197) dispatches only mesh/vector formats and has no `--render`/`--imgsize`
/`--camera` flags; nothing in `web/src` or `desktop/` calls `toDataURL` or any
capture path. OpenSCAD users script `openscad -o out.png --imgsize --camera`
for thumbnails, documentation, and CI visual diffs — and Thingiverse-style
workflows expect it.

### Work items

1. **Playground/desktop screenshot button (trivial first step)** — a "Copy
   image / Save PNG" action on the viewer canvas. Needs
   `preserveDrawingBuffer: true` on the WebGL context (or a
   render-then-capture in the same frame) plus `canvas.toBlob`. Half a day
   including UI.
2. **CLI `--render out.png --imgsize WxH --camera …`** — headless, so no GL:
   a small software rasterizer over the already-tessellated mesh
   (orthographic + perspective, z-buffer, flat shading with the viewer's
   default palette and background). The mesh is already in memory and models
   are small by GPU standards; a few hundred lines, no new heavyweight deps
   beyond the `png` crate (already in-tree via `quito-eval`'s heightmap
   support). Camera syntax should accept OpenSCAD's documented
   `--camera=tx,ty,tz,rx,ry,rz,dist` and `--viewall`/`--autocenter` so
   existing scripts port unchanged.
3. **Why it compounds**: PNG output + the golden corpus from track A enables
   *visual* regression diffs later, and gives the README/social-preview
   pipeline a native tool.

**Effort: GUI trivial; CLI medium (~3–4 days).**

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

> A user opens a multi-file OpenSCAD project in Quito desktop (saving with ⌘S,
> inline error squiggles, and `color()`/`#`/`%` in the preview already work —
> B1, B2, B3), and exports both an STL and a PNG thumbnail — without touching
> OpenSCAD. (COMPAT.md divergence #5 is now closed.)
