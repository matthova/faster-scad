# Changelog

## 0.9.1

### Patch Changes

- [#84](https://github.com/matthova/openrscad/pull/84) [`1c0b6bb`](https://github.com/matthova/openrscad/commit/1c0b6bb4818b19cafaf51a80d6712b655640bdde) Thanks [@matthova](https://github.com/matthova)! - label the nav-cube gnomon axes with X/Y/Z at each stub's tip (widget slightly enlarged so the labels aren't clipped)

- [#84](https://github.com/matthova/openrscad/pull/84) [`1c0b6bb`](https://github.com/matthova/openrscad/commit/1c0b6bb4818b19cafaf51a80d6712b655640bdde) Thanks [@matthova](https://github.com/matthova)! - shift the accent from amber to a pure yellow (chrome, viewer mesh, favicons) — no orange tone

## 0.9.0

### Minor Changes

- [#75](https://github.com/matthova/openrscad/pull/75) [`57f2cec`](https://github.com/matthova/openrscad/commit/57f2cec9b1e6f1da1910b8b458f74dfd8b254608) Thanks [@matthova](https://github.com/matthova)! - web: add an /about marketing page — a render shootout (per-model charts + table of OpenRSCAD vs OpenSCAD CGAL/Manifold, geomean ~29× / ~3.9×) and OS-autodetecting desktop download CTAs (macOS Apple Silicon/Intel, Windows, Linux), plus a link to every other option on the GitHub release page

- [#75](https://github.com/matthova/openrscad/pull/75) [`57f2cec`](https://github.com/matthova/openrscad/commit/57f2cec9b1e6f1da1910b8b458f74dfd8b254608) Thanks [@matthova](https://github.com/matthova)! - web: add a dismissable callout inviting browser users to download the desktop app (remembers dismissal across sessions)

- [#82](https://github.com/matthova/openrscad/pull/82) [`45cd508`](https://github.com/matthova/openrscad/commit/45cd50801bc4361717f8752ed381a0c691eaa67a) Thanks [@matthova](https://github.com/matthova)! - Rename the project from Quito to OpenRSCAD. This renames the published npm engine
  package (`quito-engine` → `openrscad-engine`), the Rust crates (`quito-*` →
  `openrscad-*`), the desktop app and its bundle identifier, and all user-facing
  branding. **Breaking:** consumers of `quito-engine` must switch to
  `openrscad-engine`, and desktop users get a fresh app identity (previous
  auto-updates do not carry over across the new bundle identifier).

- [#80](https://github.com/matthova/openrscad/pull/80) [`2c84e79`](https://github.com/matthova/openrscad/commit/2c84e79b073e238d5307969aa2398465b2bf3d77) Thanks [@matthova](https://github.com/matthova)! - web: the marketing page is now the site root (/openrscad/) and the playground moved to /openrscad/playground (breaking: bookmarks to the old editor root now land on the marketing page; use /playground)

### Patch Changes

- [#77](https://github.com/matthova/openrscad/pull/77) [`1e9ab2f`](https://github.com/matthova/openrscad/commit/1e9ab2f79c6f1b65aa4cb5ff1ee9e85c6e49c22f) Thanks [@matthova](https://github.com/matthova)! - serialize function values to OpenSCAD's `function(params) body` text (str/echo), fix `is_num(nan)` and exact `tan()` at 45° multiples, and honor BOSL2 `expect_success` error tests — BOSL2 function-suite coverage rises from 428/513 to 503/513

- [#76](https://github.com/matthova/openrscad/pull/76) [`45ee723`](https://github.com/matthova/openrscad/commit/45ee723e216ce662713dbb1927ee2ad6cb235139) Thanks [@matthova](https://github.com/matthova)! - web: the editor's desktop-app callout now downloads the right build for your OS directly (macOS Apple Silicon/Intel, Windows, Linux) instead of routing through /about, and is hidden on phones/tablets where there's no desktop build; the "OpenRSCAD playground" wordmark now links to /about

- [#81](https://github.com/matthova/openrscad/pull/81) [`7a5c410`](https://github.com/matthova/openrscad/commit/7a5c410864682fdecd8a54117a3533257ec3e417) Thanks [@matthova](https://github.com/matthova)! - web: the viewer's floor grid, colored X/Y/Z axes, and their numeric unit labels now extend across the whole viewport so nothing visibly ends on screen as you orbit or pan. The grid cell size snaps to 1-2-5-10 steps (…50, 20, 10, 5, 2, 1…) instead of only powers of ten, so it switches to a smaller unit much sooner and more evenly as you zoom in. The navigation cube gains a matching colored X/Y/Z axis gnomon.

- [#79](https://github.com/matthova/openrscad/pull/79) [`dac9632`](https://github.com/matthova/openrscad/commit/dac9632ffea1bd3cf2e7d379a458c8e841c280cc) Thanks [@matthova](https://github.com/matthova)! - 3D `minkowski()` now distributes over `union()`, so a concave shape built from a union of convex parts is computed exactly (e.g. `minkowski(){ union(){ cube A; cube B; } cube; }`) instead of as its convex hull. A genuinely concave leaf mesh still falls back to the convex approximation with a warning.

- [#78](https://github.com/matthova/openrscad/pull/78) [`c12702c`](https://github.com/matthova/openrscad/commit/c12702c737a1d78e9056e1c92074f7b4c9a31156) Thanks [@matthova](https://github.com/matthova)! - `text(font=)` now selects across the bundled Liberation family — Sans/Serif/Mono in Regular/Bold/Italic/Bold Italic (e.g. `font="Liberation Serif:style=Bold"`) — matching OpenSCAD's glyphs exactly; unknown families still fall back to Liberation Sans with a warning. Bundling the full family grows the wasm engine by ~3.6 MB.

- [#73](https://github.com/matthova/openrscad/pull/73) [`f41da66`](https://github.com/matthova/openrscad/commit/f41da66db37d3042d5e2d5c0e0fa8334fe6b20a1) Thanks [@matthova](https://github.com/matthova)! - fix scope assignments to match OpenSCAD's last-write-wins semantics: a read of a variable reassigned later now sees its final value (`p = 1; q = p; p = 5;` gives `q == 5`), while variables introduced later are still not forward-referenced

- [#73](https://github.com/matthova/openrscad/pull/73) [`f41da66`](https://github.com/matthova/openrscad/commit/f41da66db37d3042d5e2d5c0e0fa8334fe6b20a1) Thanks [@matthova](https://github.com/matthova)! - warn on dead (overwritten) assignments in your own source, add `rotate_extrude(start=)` for partial sweeps, and lock `linear_extrude(v=)` oblique extrudes to the geometry oracle

## 0.8.0

### Minor Changes

- [#70](https://github.com/matthova/faster-scad/pull/70) [`cdf2599`](https://github.com/matthova/faster-scad/commit/cdf259948bdb0b7f5c2e5ecd87b0338a1f6be9f2) Thanks [@matthova](https://github.com/matthova)! - Web UI (M9 Phase 2 — the signature): **isolate and dimension any part of your model without touching the code.** The dock gains an **Objects** section listing the parts your script produced (with triangle counts); click one — or click a face in the viewport — and every other part is hidden, the ISO dimension callouts retarget to that part's bounding box, and the editor jumps to its source. The readout reports the isolated subset's triangles and extent (never volume — a subset of leaves isn't a closed solid). Escape (or "Show all") restores the whole model. Selection is viewport-only and non-destructive: no re-render, no edit to your file, and because it's world-space geometry it travels into PNG captures — a detail drawing of a sub-assembly, straight from code. The OpenSCAD engine has no provenance, so the section explains that instead of vanishing.

- [#70](https://github.com/matthova/faster-scad/pull/70) [`cdf2599`](https://github.com/matthova/faster-scad/commit/cdf259948bdb0b7f5c2e5ecd87b0338a1f6be9f2) Thanks [@matthova](https://github.com/matthova)! - Web UI (M9 Phase 6): the playground is now usable on narrow screens. At ≥1024px the three resizable columns stand; below that the editor and viewer become a **Code ⎪ Model** segmented switch (Model — viewer + customizer — shown first, since read-and-tweak is the point at those widths), touch targets grow to 44px, the toolbar scrolls instead of overflowing, and phone widths shed secondary chrome. The core loop — edit, render, read the numbers, drag a parameter — works with no horizontal scrollbar at 1024, 820, and 480px. (Previously there were zero media queries and the app was unusable below desktop width.)

- [#70](https://github.com/matthova/faster-scad/pull/70) [`cdf2599`](https://github.com/matthova/faster-scad/commit/cdf259948bdb0b7f5c2e5ecd87b0338a1f6be9f2) Thanks [@matthova](https://github.com/matthova)! - Web UI (M9): add a **theme toggle** — Auto / Light / Dark. The playground previously always followed the OS appearance with no way to override it; now the choice persists across reloads. Available from the command palette (⌘K → "Theme: …").

- [#70](https://github.com/matthova/faster-scad/pull/70) [`cdf2599`](https://github.com/matthova/faster-scad/commit/cdf259948bdb0b7f5c2e5ecd87b0338a1f6be9f2) Thanks [@matthova](https://github.com/matthova)! - Web UI (M9 Phase 4): the toolbar is reorganized into grouped menus so it fits on **one row** (it previously wrapped to two even at 1280px). Controls now live under **Project ▾** (New, Import…, Share, Download .scad; Open/Save/Save As on desktop), **Quality ▾** (Draft/Normal/Fine/Custom with `$fn`/`$fa`/`$fs`, current preset shown in the label), **Export ▾** (a split button: one-click export in the current format, with the format list, PNG, and animation Frames in the caret menu), and **? ▾** (help, theme, GitHub, version). The animation transport (play, `$t` scrubber, FPS, Steps) moves to a strip **below the viewport** that stays collapsed until your script uses `$t`. New: browser **Import…** via a file picker (was drag-only). Menus close on action and open on the group triggers; the row no longer wraps at 1024px.

### Patch Changes

- [#70](https://github.com/matthova/faster-scad/pull/70) [`cdf2599`](https://github.com/matthova/faster-scad/commit/cdf259948bdb0b7f5c2e5ecd87b0338a1f6be9f2) Thanks [@matthova](https://github.com/matthova)! - Web UI (M9): accessibility — the app now exposes a `<main>` landmark and a page heading, the code editor has an accessible name, the engine and Fast toggles report their pressed state, and the status bar announces render outcomes to screen readers. The automated axe-core CI gate is tightened to enforce these (landmark, heading, and control-name rules) on every PR.

- [#70](https://github.com/matthova/faster-scad/pull/70) [`cdf2599`](https://github.com/matthova/faster-scad/commit/cdf259948bdb0b7f5c2e5ecd87b0338a1f6be9f2) Thanks [@matthova](https://github.com/matthova)! - Web UI (M9): the **Custom** render-quality preset now exposes `$fa` (max fragment angle) and `$fs` (max fragment size) inputs alongside `$fn`. These were already persisted, validated, and injected into renders but had no UI; the tolerance knobs are how you match OpenSCAD's 12°/2 mm defaults on curves that don't set `$fn`.

- [#70](https://github.com/matthova/faster-scad/pull/70) [`cdf2599`](https://github.com/matthova/faster-scad/commit/cdf259948bdb0b7f5c2e5ecd87b0338a1f6be9f2) Thanks [@matthova](https://github.com/matthova)! - Web UI (M9 Phase 0): fix four defects. Dropped **binary STL** files are now detected and refused with a clear message instead of being UTF-8-mangled and fed to the parser as garbage (ASCII STL still imports). In the browser, **⌘S** now downloads the active `.scad` instead of being a swallowed no-op, and works even when the editor isn't focused. The **Display ▾** indicator now lights when the section plane or dimensions are on (it previously ignored both, so enabling the geometry-hiding section plane left the menu dark). Switching to the **OpenSCAD engine** on a slow connection no longer aborts the first render with a misleading "model too complex" message — the ~10 MB download runs outside the render watchdog with a visible downloading banner. Also corrected the `#` highlight-modifier label in the Model panel (was mislabelled `!`).

  Accessibility: keyboard focus is now always visible (a `:focus-visible` ring that clears 3:1 in both themes — there were previously zero focus styles), light-theme text colours (`--muted`, `--warn`, the amber accent in text/border roles) were darkened to meet WCAG contrast, control boundaries and the floating **⤢ Fit** button use a stronger border token so they don't vanish, the tab close (✕) is no longer a faint 2.22:1 glyph, and animations honour `prefers-reduced-motion` (including the viewport fly-to camera).

- [#70](https://github.com/matthova/faster-scad/pull/70) [`cdf2599`](https://github.com/matthova/faster-scad/commit/cdf259948bdb0b7f5c2e5ecd87b0338a1f6be9f2) Thanks [@matthova](https://github.com/matthova)! - Web UI (M9 Phase 1b): the **orthographic projection** toggle and the **console** drawer's open state and severity filter are now remembered across reloads (they previously reset every session). Internally, persisted toggles whose value the render loop reads (Fast preview, the engine choice, editor↔preview linking) now flow through a single `usePref` hook that keeps React state, the shadow ref, and localStorage in sync atomically — closing a class of bug where a toggle could light up while renders silently ignored it.

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
