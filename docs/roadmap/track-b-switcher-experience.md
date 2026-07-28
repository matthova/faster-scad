# Track B — "Switcher Experience" (proposed M7)

**Goal:** an OpenSCAD user can adopt Quito (desktop or CLI) for a real project
without hitting a workflow wall. Every item below is a confirmed gap where
Quito breaks a daily habit an OpenSCAD user already has.

**Why after track A:** each of these polishes the adoption funnel. Funneling
users into an engine that still silently mis-rotates `rotate(45,[1,1,0])`
(track A, item A1) converts adopters into detractors — trust first, comfort
second. Sequenced as M7, it inherits a geometry engine users can rely on.

**Effort:** ~2 weeks remaining.

> **Done:** B1 (desktop Save source — `save_source`/`watch_files` commands, ⌘S /
> Save As, native menu, dirty-tab indicator, watcher reentrancy, `.scad` file
> association) shipped and is removed from this doc.

---

## B2. Inline editor diagnostics (error squiggles)

### Current state

Errors appear only in the status bar and console panel. Verified:
`web/package.json` has no `@codemirror/lint` dependency and nothing in
`web/src` calls `setDiagnostics`. The frustrating part: **spans already exist
end-to-end and are being thrown away** — `quito-syntax` parse errors carry
`e.span` (byte range), but `crates/quito-wasm/src/lib.rs:246` and
`desktop/src-tauri/src/lib.rs:178,254` flatten it into
`format!("parse error: {} (at {}..{})")`, and `web/src/engineWorker.ts`
exposes only `error: string`. The README's "parse errors surfaced inline" is
currently an overstatement.

### Work items

1. **Structured error type across the boundary** — change the wasm
   `RenderResponse` and the Tauri IPC payload from `error: String` to
   `{ message, file, start, end, kind }` (keep a formatted string alongside
   for the console). Parse errors have exact spans today; eval errors should
   carry the best span available (call-site of the failing builtin) and can
   improve incrementally.
2. **`@codemirror/lint` integration** — add the dep, convert byte offsets to
   the active document's positions, `setDiagnostics` on each render response,
   clear on success. Multi-file wrinkle: an error in a library tab should
   badge that tab and jump-to on click, not squiggle the wrong document.
3. **Warnings too** — the console already receives warnings; give them spans
   where available and render as yellow squiggles. This pays off double when
   track A's minkowski warning (A5) and hoisting lint land.

**Effort: small-medium (~2–3 days). High visible payoff; also makes the README
claim true.**

---

## B3. `color()`, `#`, `%` — visual semantics in the preview

### Current state

`color()` is evaluated as a passthrough group (`quito-eval/src/lib.rs:589`) —
the argument is discarded, so every model renders uniform orange, and exports
that could carry color (3MF) don't. The `#` (highlight) and `%` (background)
modifiers are parsed and passed through with no visual treatment —
`quito-eval/src/lib.rs:553` has the comment "`#` highlight and `%` background
are visual-only; passed through in M0", and `web/src/viewer.ts` has no
modifier material handling. This is COMPAT.md divergence **#5, the last open
M0 divergence**, and it's what makes multi-part models (assemblies, print-plate
layouts) illegible in the preview.

### Work items

1. **IR**: add a `Color(rgba)` node (evaluated: named colors, hex strings,
   `[r,g,b]`/`[r,g,b,a]` vectors — OpenSCAD's documented forms) and
   highlight/background flags on nodes (or wrapper nodes) for `#`/`%`.
2. **Mesh channels**: booleans destroy per-face provenance, so follow
   OpenSCAD's semantics — color applies to the *result* of the subtree it
   wraps. Render colored subtrees as separate meshes: partition the CSG root
   into color groups, render each, and ship `Vec<(Mesh, Rgba)>` to the viewer.
   The geometry cache keys on structure, so this splits cleanly along cache
   boundaries. Booleans *across* color groups (difference of a red from a blue)
   resolve to the parent's color — verify the exact precedence against the
   oracle visually.
3. **Viewer**: per-mesh materials in three.js; `#` renders the subtree
   additionally as a translucent red overlay, `%` renders it translucent-gray
   and excluded from exports (matching documented semantics).
4. **Exports**: 3MF supports per-object color — wire the color groups into the
   existing 3MF writer. STL/OFF/OBJ unchanged.
5. Closes COMPAT #5; update the divergence register.

**Effort: medium (~1 week) — touches IR, both kernel paths, wasm/IPC surface,
viewer, and one exporter.**

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

> A user opens a multi-file OpenSCAD project in Quito desktop (saving with ⌘S
> already works — B1), edits with inline error squiggles, sees `color()`/`#`/`%`
> rendered correctly in the preview, and exports both an STL and a PNG thumbnail —
> without touching OpenSCAD. COMPAT.md divergence #5 is closed; the README's
> "parse errors surfaced inline" claim is true.
