# Changelog

## 0.7.0

### Minor Changes

- [#68](https://github.com/matthova/faster-scad/pull/68) [`46a3e3c`](https://github.com/matthova/faster-scad/commit/46a3e3cfcf39826b1b75c58f8e7cf09c723a7fe3) Thanks [@matthova](https://github.com/matthova)! - Web UI capabilities: a **section (clipping) plane** (Display ▾ → Section, with X/Y/Z axis and a position slider) cuts the model so you can see inside; **drag-and-drop import** loads local .scad/data files (a dropped .scad renders immediately; binary STL/3MF/PNG surface a message rather than failing silently); a **help / shortcut sheet** (the ? button) surfaces the keyboard shortcuts and previously-undiscoverable features (nav cube, $vp scripting, BOSL2 auto-fetch, modifier characters); and autosave now **warns when browser storage is full** instead of silently dropping your work.

- [#68](https://github.com/matthova/faster-scad/pull/68) [`46a3e3c`](https://github.com/matthova/faster-scad/commit/46a3e3cfcf39826b1b75c58f8e7cf09c723a7fe3) Thanks [@matthova](https://github.com/matthova)! - Web UI: add ISO dimension callouts (the "signature" mode). Toggle **Dimensions** from Display ▾ to annotate the model's bounding box with real world-space drafting callouts — extension lines, arrowheads, and the width/depth/height in millimetres — and the grid's numeric tick labels step aside so the two don't collide. Because the callouts are world-space geometry, they travel into PNG captures. Off by default.

- [#68](https://github.com/matthova/faster-scad/pull/68) [`46a3e3c`](https://github.com/matthova/faster-scad/commit/46a3e3cfcf39826b1b75c58f8e7cf09c723a7fe3) Thanks [@matthova](https://github.com/matthova)! - Web UI: add a render-quality control ($fn/$fa/$fs: Draft/Normal/Fine/Custom), a render-integrity badge in the status bar (EXACT / FAST PREVIEW / DEGRADED), and click-to-source on diagnostics that resolve to a span. The status bar now holds its last-good numbers so they no longer blink during animation playback, and the desktop status bar reports the real engine version instead of "native".

- [#68](https://github.com/matthova/faster-scad/pull/68) [`46a3e3c`](https://github.com/matthova/faster-scad/commit/46a3e3cfcf39826b1b75c58f8e7cf09c723a7fe3) Thanks [@matthova](https://github.com/matthova)! - Web UI restructure: give every control one obvious home. Zoom-to-fit replaces the I/F/T/R + Reset view toolbar buttons (the nav cube already does presets). A Display ▾ popover holds projection, link, and new grid/edge toggles. The right column becomes a dock with collapsible Parameters and Model sections (the Model section surfaces vertices, area, per-color parts, integrity, and libraries) that folds to a spine when a script has no params. Editor / dock / console are resizable with persisted sizes, the console gains severity filter chips, and a ⌘K command palette plus ⌘↵ / ⌘J / ⌘⇧F shortcuts land.

### Patch Changes

- [#68](https://github.com/matthova/faster-scad/pull/68) [`46a3e3c`](https://github.com/matthova/faster-scad/commit/46a3e3cfcf39826b1b75c58f8e7cf09c723a7fe3) Thanks [@matthova](https://github.com/matthova)! - Web UI: align the editor's chrome (background, gutters, active line, brackets, autocomplete) to the app's design tokens so the editor no longer reads as a separate VSCode pane — the seam between editor, viewport, and dock is gone. Syntax colors are unchanged. Light mode gets a designed three-elevation palette (crisp white content, cool-grey chrome) instead of literal white.

## 0.6.0

### Minor Changes

- [#66](https://github.com/matthova/faster-scad/pull/66) [`61d0e51`](https://github.com/matthova/faster-scad/commit/61d0e51cdc3981f633a5b93861f0689d17afc675) Thanks [@matthova](https://github.com/matthova)! - extend the Quito⇆OpenSCAD engine toggle to the desktop app. The toolbar toggle now appears on desktop too: "Quito" uses the native C++ engine, and "OpenSCAD" renders with a **locally-installed OpenSCAD** (its fast Manifold backend) when one is available — found via the `QUITO_OPENSCAD` override, `PATH`, or the standard per-platform install locations — shelling out to export binary STL (exact) or colored OFF ($preview, F5-style). If no local OpenSCAD is installed it transparently falls back to the vendored OpenSCAD wasm build in the webview, so the toggle always works. Includes resolve from open tabs plus the file's directory (OPENSCADPATH), and exports write the OpenSCAD-produced geometry via the native save dialog. Browser behavior is unchanged (wasm engine).

## 0.5.0

### Minor Changes

- [#64](https://github.com/matthova/faster-scad/pull/64) [`62c6aab`](https://github.com/matthova/faster-scad/commit/62c6aab56978ae172bf542345815dfb7e1ee9398) Thanks [@matthova](https://github.com/matthova)! - add a "Fast" preview toggle that renders unions by concatenation instead of the CSG kernel — much faster on union-heavy models (skips the kernel's costliest work), at the cost of a non-watertight on-screen mesh. Differences, intersections and hulls still resolve exactly, so holes and clips look correct; exports and reported volume always use the exact, watertight path.

- [#65](https://github.com/matthova/faster-scad/pull/65) [`cfee417`](https://github.com/matthova/faster-scad/commit/cfee4178f7174a951f5e6242393c2b1d75c5f61e) Thanks [@matthova](https://github.com/matthova)! - add a toolbar toggle to switch the web playground's render engine between Quito and actual OpenSCAD. The OpenSCAD path runs the official OpenSCAD 2025.03.25 WebAssembly build (Manifold backend) in a worker, loaded lazily so its ~9.6 MB wasm is only downloaded when selected. Handy for comparing our output against upstream. On the OpenSCAD engine the "Fast" toggle acts like OpenSCAD's F5 preview — a colored render showing `color(...)` — while Fast off gives a plain exact render. Limitations while on the OpenSCAD engine: no customizer or editor↔preview linking (Quito-only), and 3D models only.

### Patch Changes

- [#63](https://github.com/matthova/faster-scad/pull/63) [`d3db472`](https://github.com/matthova/faster-scad/commit/d3db4720b29e238910e7f5cb3f73385a7d8b37b6) Thanks [@matthova](https://github.com/matthova)! - Add an animated BOSL2 gear-train demo, and fix two engine bugs it exposed: (1) an omitted function parameter now correctly shadows a same-named global (as `undef`) even inside `assert(...) expr` guard bodies, so BOSL2 gears.scad idioms like `circular_pitch()` no longer trip a spurious assertion; (2) `linear_extrude` now drops consecutive duplicate vertices, so profiles that emit zero-length edges (e.g. BOSL2's `rack2d`) produce manifold solids instead of degrading to un-combined geometry under boolean union.

- [#60](https://github.com/matthova/faster-scad/pull/60) [`dc2f96d`](https://github.com/matthova/faster-scad/commit/dc2f96d3126e9499764ceb6fe0718239b927307e) Thanks [@matthova](https://github.com/matthova)! - add a GitHub link icon to the toolbar (opens the repo in the system browser, works in web and desktop)

- [#62](https://github.com/matthova/faster-scad/pull/62) [`f2aa6bf`](https://github.com/matthova/faster-scad/commit/f2aa6bf486b8637ca5d3ef4d4d3eb6c9a3976000) Thanks [@matthova](https://github.com/matthova)! - default the export format to 3MF for multi-color models (until you pick a format yourself), so colors aren't silently dropped by STL

## 0.4.1

### Patch Changes

- [#58](https://github.com/matthova/faster-scad/pull/58) [`18b4553`](https://github.com/matthova/faster-scad/commit/18b4553aa964277b4a45548898abaa912b7ed478) Thanks [@matthova](https://github.com/matthova)! - fix crash-recovery so a too-heavy model can't freeze the app on every launch: the render watchdog no longer clears its own recovery sentinel, and safe mode now survives repeated relaunches until a render actually finishes (previously a render heavier than the 20s watchdog re-triggered the freeze on startup, since the watchdog's own timeout wiped the "skip auto-render" flag)

## 0.4.0

### Minor Changes

- [#56](https://github.com/matthova/faster-scad/pull/56) [`d6abf41`](https://github.com/matthova/faster-scad/commit/d6abf4158500549b9c6846c1eb82db180b275613) Thanks [@matthova](https://github.com/matthova)! - recover from heavy-geometry render freezes: a render-in-progress indicator with a Stop button, a watchdog that auto-stops runaway renders, and a startup recovery banner that skips auto-rendering a project whose last render never finished (so a too-heavy script no longer freezes the app on every reload)

## 0.3.0

### Minor Changes

- [#54](https://github.com/matthova/faster-scad/pull/54) [`2776978`](https://github.com/matthova/faster-scad/commit/2776978288df61fabe157a96026665d0c65a62ff) Thanks [@matthova](https://github.com/matthova)! - viewer: add a zoom-adaptive reference grid with numeric X/Y/Z axis labels and ruler ticks (spacing steps by powers of ten as you zoom), plus a draggable navigation cube in the top-right — drag it to orbit, or click a face, edge, or corner to fly to that face-on, 45°, or isometric view

## [0.2.0](https://github.com/matthova/faster-scad/compare/v0.1.1...v0.2.0) (2026-08-01)

### Features

- **geom,web:** render non-manifold models via weld + graceful CSG degradation ([#47](https://github.com/matthova/faster-scad/issues/47)) ([011fcfb](https://github.com/matthova/faster-scad/commit/011fcfbc2bcb7139532814a6cf0f926ebb75ab93))
- **npm:** publish the wasm engine as `quito-engine` ([#48](https://github.com/matthova/faster-scad/issues/48)) ([4b2eda7](https://github.com/matthova/faster-scad/commit/4b2eda7f38d7348a9cc1b693f0ae4ac2d4ba68f3))
