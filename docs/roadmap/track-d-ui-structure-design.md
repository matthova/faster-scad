# OpenRSCAD web UI — feature structure & essential additions

## Context

The OpenRSCAD playground has grown feature-by-feature through M0–M7 and every new
control landed in the same place: one non-wrapping flex row. `.actions`
(`web/src/App.tsx:1496-1689`) now holds **~20 controls spanning eight unrelated
jobs** — project, file, camera, display, engine, render mode, animation, export.
There is no second place to put anything, so the next feature makes it worse.

Underneath, the same compression shows up structurally: `App.tsx` is **1,866
lines** and is the entire app (3 components exist in total); the "design system"
is **8 color tokens and nothing else** — no spacing, radius, type, elevation, or
z-index scale; nothing is resizable; there are **zero `@media` queries**; and the
whole frontend has **no ESLint, no component/DOM tests, and no Playwright**
(`web/package.json:33-45`).

Meanwhile the engine already produces things the UI throws away (`area`,
`vertexCount`) and supports things the UI cannot reach at all (`$fn`/`$fa`/`$fs`).
The crash-recovery banner literally instructs the user to "lower `$fn`"
(`App.tsx:1696-1698`) **with no control to do it** — a broken promise in
production text.

**Outcome wanted:** a home for every existing feature, room for the next one, a
real token system, and the handful of missing controls that a code-CAD tool
cannot honestly ship without.

> Every factual claim below was verified against source by an adversarial pass.
> Corrections from that pass are folded in.

---

## 1. What exists today

Grouped by *job*, which is the grouping the current UI does not have.

| Job | Features today | Where |
| --- | --- | --- |
| **Project** | New, Examples (10, flat `<select>`), Share (`#code/` lz-string link, carries anim state), `.scad` download (active tab only), Open…/Save/Save As (desktop) | `App.tsx:1497-1531`, `share.ts`, `examples.ts` |
| **Files** | Tab strip: switch, add, rename (`window.prompt`), delete, dirty dot (desktop), diagnostic badge; multi-file `include`/`use` with BFS closure + BOSL2 CDN auto-fetch | `App.tsx:1732-1786`, `library.ts:13-111` |
| **Editor** | CodeMirror 6, Lezer OpenSCAD grammar, autocomplete, signature help, lint squiggles, fold, search, VSCode Dark+/Light+ theme | `lang/*`, `App.tsx:542-593` |
| **Parameters** | Customizer (checkbox/slider/number/text/dropdown/vector, grouped), named preset sets, import/export OpenSCAD `.json`, reset | `CustomizerPanel.tsx` |
| **Camera / display** | I/F/T/R presets (4 of 7), Reset view, Persp/Ortho, nav view cube (all 7 + edges/corners, fly-to), Link (editor↔preview highlight), adaptive power-of-ten ruler grid + tick labels + axis triad | `App.tsx:1532-1569`, `viewer.ts` |
| **Render** | Render, Stop, Fast preview toggle, OpenRSCAD⇆OpenSCAD engine toggle, 150 ms debounce, 20 s watchdog, crash-recovery sentinel + banner | `App.tsx:1585-1598`, `engine.ts`, `project.ts:65-103` |
| **Animation** | Play/pause, `$t` slider, `$t` readout, FPS, Steps, Frames→zip, `$vp*` camera round-trip | `App.tsx:1603-1654`, `viewer.ts:1027-1080` |
| **Output** | Export (STL/OFF/OBJ/3MF/AMF, or DXF/SVG when 2D) with auto-format, PNG capture, frames zip | `App.tsx:1383-1481`, `stl.ts` |
| **Diagnose** | Editor squiggles, tab badges, console drawer (echo/warn/error/geom-error), status bar (triangles · dims · volume · ms), engine version | `App.tsx:1806-1863` |
| **Platform** | PWA offline precache, OS-driven theme, desktop update banner, native menu, file watching | `vite.config.ts:9-41`, `UpdateBanner.tsx` |

### Verified gaps

- `area` and `vertexCount` cross the wasm boundary every render and are **dropped
  on the floor** (`engineWorker.ts:151,153` → `App.tsx:228-243` has no field for either).
- **No `$fn`/`$fa`/`$fs` control anywhere.**
- **No app keyboard shortcuts** for Render / Stop / console. Only CodeMirror
  defaults, `Mod-s`/`Mod-Shift-s`, and a global Escape that dismisses the pick
  highlight (`App.tsx:685-691`).
- **Nothing is resizable** — `.workspace` columns, `.params { width: 288px }`,
  `.console { height: 160px }` are hardcoded (`index.css:147,247,420`).
- **No manual theme toggle** (`prefers-color-scheme` only).
- **No drag-and-drop, no file open in the browser.** The only `<input type="file">`
  is the preset importer (`CustomizerPanel.tsx:96`).
- **Binary payloads are unreachable in the browser** — `MapResolver::load_bytes`
  serves tab text only (`crates/openrscad-wasm/src/lib.rs:324-327`), so binary STL,
  3MF, and PNG `surface()` heightmaps can't be imported. (ASCII STL, AMF, OFF,
  OBJ, DXF, SVG *do* work from a text tab.)
- **Console lines are plain text**, not clickable, though structured spans exist
  in a parallel `diagnostics` array (`App.tsx:1090-1101, 1208-1217`).
- Only 4 of 7 camera presets are in the toolbar; `back`/`left`/`bottom` are
  reachable only via the view cube.
- **Storage quota is swallowed** (`project.ts:27-29`) — a large project silently
  stops autosaving with no warning. Data-loss bug.
- Desktop status bar hardcodes `"native"` as the engine version even though the
  `engine_version` Tauri command exists and is registered but unused
  (`desktopEngine.ts:135`, `desktop/src-tauri/src/lib.rs:1067-1069`).

---

## 2. Structure

### The principle

**Re-home every control next to the thing it acts on, and add a command palette
as the release valve** so visible chrome can stay small without losing
discoverability.

Three placements were considered and **rejected** after adversarial review —
recording them so they aren't re-proposed:

- ✗ *A floating control rail over the viewport.* The viewport is already only
  ~41–44% of window width; a rail either eats canvas or hover-reveals, making the
  most-used control a two-step.
- ✗ *A new toolbar strip above the viewport.* Steals the only axis the viewport
  has left (height) and sits directly over the nav cube, which already does
  orientation better.
- ✗ *Moving triangles/dims/volume out of the status bar into a tab.* The core
  loop is "drag a slider → read the new volume." Tabbing that is a click **per
  iteration**. Those numbers stay where they are.

### Zones

| Zone | Holds | Why |
| --- | --- | --- |
| **Topbar** | `New` · `Examples` · `Share` · `.scad` (or `Open…`/`Save`) ‖ `Display ▾` · `Quality ▾` · `OpenRSCAD/OpenSCAD` · `Fast` ‖ `PNG` · `Export ▾fmt` ‖ `⌘K` · `?` · theme · GitHub | Source-out actions left, model-out actions right. `Fast` and the engine toggle stay **one-click and visible** — they're comparison gestures you flip while staring at the model, not settings. `Export` stays **one click with its auto-chosen format** — it's the terminal action of the product. |
| **`Display ▾` popover** | Persp/Ortho, Link (editor↔preview), grid, axes, edge overlay, **Dimensions**, `%`-background visibility | Set-and-forget display state. Safe to hide; hot-path controls are not. |
| **`Quality ▾` popover** | Draft / Normal / Fine / Custom → `$fn`/`$fa`/`$fs` | New. See §4.1. |
| **Editor column header** | File tabs (switch / add / rename / delete / dirty / diag badge) | Unchanged. |
| **Viewport** | Canvas, nav cube, **dimension callouts**, `Fit` button beside the cube | **Delete the I/F/T/R buttons and `Reset view`.** The cube already does all 7 presets plus edges and corners with animated fly-to (`viewer.ts:987-1015`) — 5 topbar controls removed for zero capability loss. Add `Fit` (frame without changing orientation), which does not exist today. |
| **Right dock** | One scrollable column, **collapsible sections — not exclusive tabs**: `Parameters` (existing customizer + presets) then `Model` (vertices, area, per-color parts with isolate/hide, render-integrity detail, libraries resolved). Collapses to a ~28px labelled spine when a script has no params. | Sections, not tabs, because you read Model *while* dragging a Parameter. Spine instead of vanishing so it's discoverable without costing 288px. |
| **Console drawer** | Resizable; severity filter chips; **click-to-source only on lines that actually resolve to a span** | Echo lines carry no spans and geom errors are prose — making every line *look* clickable is worse than none. |
| **Status bar** | `Render`/`Stop` · triangles · dims · volume · ms · **render-integrity state** · console counts · engine version | Keeps today's glanceable four. Two fixes: hold last-good values across renders and reserve column widths so it stops reflowing (see §4.2). |
| **Animation transport** | Stays in place for now; moves to a persistent strip under the viewport **only once the engine reports whether `$t` was read** | Substring-sniffing the source (the existing `$vp` trick at `App.tsx:482`) fires on comments and misses `$t` inside an included library. Don't move it until it can move correctly. |
| **Command palette** | Every action, one registry | See §4.3. |

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ OPENRSCAD  main.scad    New Examples Share .scad ‖ Display▾ Quality▾ OpenRSCAD Fast ‖ PNG Export▾ ‖ ⌘K ? ◐ ⌸ │
├──────────────────────────┬───────────────────────────────────┬───────────────────────┤
│ main.scad ● helpers.scad+│                            ┌────┐ │ ▾ PARAMETERS      ⟲   │
│──────────────────────────│                            │cube│ │   Dimensions          │
│ 1  $fn = 48;             │                            └────┘ │     width  [   40 ]   │
│ 2  module bracket() {    │         ╱▔▔▔▔▔▔╲            ⤢ Fit │     depth  [   20 ]   │
│ 3    difference() {      │        │  model  │                │   Fillets             │
│ 4      cube([w,d,h]);    │         ╲▁▁▁▁▁▁╱                  │     r   ━━●━━   3.0   │
│ 5      cylinder(r=3);    │   ├──────  40.00  ──────┤         │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │
│ 6    }                   │   ╌╌╌╌ adaptive ruler grid ╌╌╌╌   │ ▾ MODEL               │
│ 7  }                     │                                   │    6 242 vertices     │
│                          │                                   │    area  2 104.55     │
│                          │                                   │    2 parts  ● ●       │
├──────────────────────────┴───────────────────────────────────┴───────────────────────┤
│ ▸ CONSOLE   All · 2 warnings · 0 errors                                        ⌃ ⌄   │
├──────────────────────────────────────────────────────────────────────────────────────┤
│ ▶ Render │ 12 480 triangles · 40×20×12 mm · vol 4 812.30 · 0.8 ms │ EXACT │ console 2 │ openrscad 0.6.0 │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Visual direction

Per the frontend-design method: color / type / layout / signature, then the
self-critique of what got changed and why.

### The edit: brave in the viewport, silent in the chrome

**Color — hold the palette, add the scales.** Keep the 8 existing tokens and the
single amber accent exactly as they are. Add the missing scales: spacing, radius,
**three surface elevations** (bg / panel / raised — today there are two), type
scale, z-index, motion. Ship this so the diff is **provably visual-neutral**.
Also pull `viewer.ts`'s ~9 hardcoded hexes (background `:248`, grid `:344`, axes
`:367`, edges `:496`, mesh `:522`, cube `:799,807,889-894`, modifiers `:1119-1122`)
into the same token source — half of them only take effect via
`rebuildCubeFaces()`/`buildGrid(force)`, so this must precede any palette work or
it gets re-hardcoded.

**Type — one token, no webfont.** Promote `ui-monospace, Menlo, monospace` from
**10 scattered literals** to one token, and get character from size, weight,
tracking, and numeral alignment rather than a new family. Extend the
uppercase / letter-spaced micro-label vocabulary the app already invented at
`index.css:262-266` — that is the beginning of a house style; use it instead of
importing one.

**Layout — the zones in §2**, with real resize handles and persisted sizes.

**Signature — dimension callouts.** The one bold move, and it goes in the
viewport, not the chrome: draw the model's bounding box as **real ISO dimension
annotation in world space** — extension lines, arrowheads, the value in a break
in the line. Toggleable from `Display ▾`, off by default.

Why this and not a status-bar flourish:

- Every part already exists. `buildGrid` draws world-space `LineSegments` with
  major/minor ruler ticks (`viewer.ts:317-436`); `makeTickLabel` makes
  camera-facing numeric sprites (`viewer.ts:441-467`); the bbox is already
  computed (`viewer.ts:504-506`).
- **It travels.** `capturePng` reads only the main renderer (`viewer.ts:1093`),
  so DOM chrome and the cube overlay are invisible to it. World-space geometry
  *is* in the PNG — every screenshot a user posts becomes unmistakably OpenRSCAD.
- It says *millimetres*. This is an app about physical objects, and nothing in the
  current identity says so.
- Turning it on becomes a **mode**: dimension callouts on, the grid's numeric
  tick labels off (the grid already has suppression logic at `viewer.ts:284-288`
  to extend). The viewport becomes a drawing. Two labels reading `120.00` and
  `120` next to each other would be a bug; suppression makes it a feature.
- It composes with the measure tool (§4.9) — click two points, get the same
  drafting callout.

Constraints for the implementer: own group in `this.scene` so `setMesh`/
`setColoredMesh` don't clobber it; `polygonOffset` on extension lines so they
don't z-fight the floor grid.

### What I changed after critique, and why

- **Cut a second accent hue.** A cool chrome accent was proposed and rejected:
  cyan is *already* load-bearing inside the viewport as the selection wash
  (`viewer.ts:46-53`) and cube hover (`viewer.ts:804-813`), where it means exactly
  one thing — "the thing you pointed at." And the premise "the chrome goes cool so
  the warm model pops" dies on the flagship examples: the rocket, gear train, and
  a 67-call polychrome Parthenon all set `color()` (`examples.ts:193-208, 276-282, 290+`).
  Amber-as-active-state (`index.css:126-130`) is the app's clearest single cue;
  splitting it collapses "clickable" and "active" into a saturation delta.
- **Cut the self-hosted webfont.** Tabular figures are a property of *every*
  monospace, `index.css:631` already reaches for `font-variant-numeric`, and the
  workbox glob is `**/*.{js,css,html,wasm,svg}` (`vite.config.ts:17`) — woff2 isn't
  in it, so the signature typeface would be the one asset that breaks offline in a
  PWA sold on offline use. It would also strand the canvas-rendered tick labels
  (`viewer.ts:441`) and cube faces (`viewer.ts:896`) on a different family.
- **Cut a render-time sparkline.** It plots the number the README calls
  sub-millisecond — a flat line with one spike — sampled at typing cadence (150 ms
  debounce, `App.tsx:527`) and strobing at 15 Hz during playback
  (`App.tsx:732-744`). Print the number and let it be small.
- **Acknowledged, not fixed:** the editor's 9-hue VSCode Dark+ palette
  (`lang/theme.ts:15-24`) fills 36% of the screen and is not token-driven. Don't
  touch the syntax colors (muscle memory), but **do** align the editor's *chrome*
  — gutter, background, active line — to the app tokens so the seam disappears.
- **Light mode must be designed, not inherited.** It's currently literal white
  (`index.css:16-26`); the three-elevation scale needs its own light values.

---

## 4. Essential additions, ranked

**1. Render quality — `$fn`/`$fa`/`$fs`.** Ranks first because the app currently
ships text telling the user to lower `$fn` with no way to do it
(`App.tsx:1696-1698`). Draft / Normal / Fine / Custom, injected the same way the
customizer injects overrides (`-D` literals, `customizer.ts:39-44`). Persist in
`prefs.ts`. Not in the share link.

**2. Stabilise the status bar.** It renders only when `status.ok && !rendering`
(`App.tsx:1822`), so during playback the numbers blink ~15×/s in a `nowrap`
`gap:16px` row (`index.css:456-465`), shifting Render/Stop/console sideways. Hold
last-good values across renders, reserve column widths, tabular numerals. This is
a prerequisite for adding anything to that bar.

**3. `ResizeObserver` on the canvas container.** `Viewer` listens only to
`window.resize` (`viewer.ts:118,188`) while `resize()` and `updateGrid()` read
`clientWidth/clientHeight`. **Every** panel-resize, drawer-drag, transport-expand,
and dock-collapse changes the canvas box without a window resize → stale
`setSize`, stretched render, wrong grid math. **Hard prerequisite** for resizable
panels and any collapsible region.

**4. Free inspector data.** `area` and `vertexCount` are already on the wire
(`engineWorker.ts:151,153`). Surface them, plus a per-colour parts list from the
`groups` JSON (which already carries colour + `solid`/`highlight`/`background`
mode). Ship this before plumbing new fields across the wasm boundary — let it
prove anyone wants the panel.

**5. Command palette + real shortcuts.** ⌘K, plus ⌘↵ render, ⌘J console, ⌘⇧F fit.
Two collisions to handle explicitly: **Escape is already bound** globally to
dismiss the pick highlight (`App.tsx:685-691`), and **⌘↵ is CodeMirror
`defaultKeymap`'s `insertBlankLine`** — the override must precede `basicSetup`
(`App.tsx:546`) in keymap precedence. Build the registry against **web actions
only**; do *not* try to unify the Tauri native menu (`App.tsx:633-656` relays a
fixed 7-case switch from Rust — that's a Rust change plus a desktop release cycle).

**6. Clickable diagnostics.** `byteToChar` exists (`App.tsx:161-172`) and lint is
wired (`App.tsx:861-869`). Highest value-to-code ratio in the list. Scope it to
the structured `diagnostics` array only.

**7. Render-integrity state.** Not a "manifold check" — the engine already
guarantees watertight output. The real, currently-invisible problem is *is what
I'm looking at the real thing?*: **Exact** / **Fast preview** (unions skipped,
volume approximate, not watertight) / **Degraded** (CSG failed, mesh shown
anyway — today only a red console line). One word in the status bar, detail in
the Model section.

**8. Resizable + collapsible panels**, persisted sizes. Needs #3 first.

**9. Measure tool + zoom-to-fit.** `Fit` frames the model without changing
orientation — auto-frame fires once (`viewer.ts:508-511`) and `Reset view` also
resets the angle, so this genuinely doesn't exist. Point-to-point measure renders
as the same drafting callout as the signature.

**10. Section / clipping plane.** The single most-missed CAD viewer feature.
No clipping code exists in `viewer.ts` today.

**11. Open a local file, incl. binary.** Drag-and-drop + picker. Text formats work
immediately; binary STL/3MF and PNG heightmaps need a `Vec<u8>` channel in
`render_with_files`.

**12. Silent-degradation surfaces.** Several failures are invisible today:
storage-quota exceeded stops autosave silently (`project.ts:27-29`); a missing
library is dropped with only an engine warning (`library.ts:98`); the OpenSCAD
engine silently has no customizer, no provenance, and is 3D-only
(`openscadWorker.ts:20-24`); desktop silently falls back to bundled wasm when no
binary is found (`desktopEngine.ts:305-311`); the ~10 MB OpenSCAD download is
mentioned only in a tooltip. Each needs a visible state.

**13. Examples browser** — thumbnails, descriptions, tags; served as JSON rather
than 1,050 lines of bundled TS.

**14. Theme toggle** (Auto / Light / Dark).

**15. Help / shortcut sheet + first-run.** `$vp*` camera scripting, BOSL2
auto-fetch, the `!`/`*` modifier characters, and the view cube are all currently
undiscoverable.

> **Responsive/mobile is explicitly out of scope.** No `@media` work is planned.
> Note that the restructure fixes the narrow-laptop overflow incidentally — the
> topbar drops from ~20 controls to ~12 by deleting I/F/T/R + `Reset view` and
> folding display state into `Display ▾`. Don't add breakpoints on top of that.

---

## 5. Sequencing

The constraint that drives this order: **zero UI safety net.** No ESLint, no
component tests, no jsdom, no Playwright, no visual regression, and a 1,866-line
`App.tsx` whose JSX return alone is ~375 lines. Every regression is currently
found by a human squinting.

| Phase | Contents | Gate |
| --- | --- | --- |
| **0 — Safety net** | Playwright + a handful of screenshot baselines (default project, an example, error state, light + dark). Prettier. | Baselines green before anything moves. |
| **1 — Tokens** | Spacing / radius / type / elevation / z-index scales; mono → one token; `viewer.ts` hexes → shared source; fix `index.css:570`'s raw `#23272f`. **Palette unchanged.** | Screenshots byte-identical. This is the whole point of doing it first. |
| **2 — Free wins, no layout change** | §4.1 Quality, §4.2 status-bar stability, §4.3 `ResizeObserver`, §4.4 `area`/`vertexCount`, §4.6 clickable diagnostics, §4.7 integrity state, desktop `engine_version` fix. | Each independently shippable. |
| **3 — Structure** | Topbar split, `Display ▾`/`Quality ▾` popovers, delete I/F/T/R + `Reset view`, add `Fit`, dock sections + spine, console drawer, resizable panels. | Needs 0 and 3. |
| **4 — Palette & light mode** | Real, considered evolution of the dark and light palettes; three-elevation values in both; editor *chrome* aligned to tokens (syntax colors untouched). | Only after 1 and 3 land — the token extraction has to be provably neutral first, or there's no baseline to judge the change against. |
| **5 — Signature (spike first)** | **Prototype dimension callouts on a branch and look at it before committing.** Throwaway quality is fine; the question is whether it reads as drafting or as clutter next to the ruler grid and axis triad. Decision gate: ship / iterate / drop. Measure tool only if the callouts land. | Viewer work, independent of 3/4 — can spike any time. |
| **6 — Capability** | Section plane, file import + `Vec<u8>` channel, examples browser, silent-degradation surfaces, help sheet. | Each its own PR. |

**Files:** `web/src/index.css` (tokens), `web/src/App.tsx` (extract the toolbar at
`1496-1689`, the tab strip at `1732-1786`, the console at `1806-1818`, the status
bar at `1820-1863`, and the `onResult` fan-out at `1070-1218` — that function is
the de-facto reducer and is the right first extraction),
`web/src/viewer.ts` (ResizeObserver, Fit, dimension callouts, token source),
`web/src/CustomizerPanel.tsx` → dock sections, `web/src/prefs.ts` (new persisted
prefs), plus new `web/src/components/` and a `commands.ts` registry.

**Preserve:** the ref-shadow pattern is load-bearing, not accidental — the mount
effect (`App.tsx:395-703`) has `[]` deps and its closures must read live values
(~40 refs mirror 22 state values). Any component split must keep live-value access
or move to a real store; don't half-migrate.

**Roadmap:** `docs/roadmap/` currently has **no open track** — M6 (track A) and M7
(track B) are both closed. This lands as **`docs/roadmap/track-d-ui-structure.md`**
in the existing format (goal / why-after / numbered items / exit criterion), and
`docs/roadmap/README.md` gets the new row. Track C (CI hardening) is still open
and untouched by this.

Suggested exit criterion: *a new user can find every feature without a tooltip
hunt; every control has one obvious home; nothing in the app fails silently; and
the topbar has room for the next feature.*

**Changesets:** one per shipped PR, not one for the track. Phase 0 and 1 are
`docs`/`refactor`-shaped and need **none** (a token extraction with byte-identical
screenshots is not user-facing). Phases 2, 3, 5, 6 each need a `minor` — new
capability. Write the summary for a release-notes reader.

---

## 6. Verification

- `cd web && npm run build:wasm && npm install && npm test && npm run build`
  (`web/engine/` is gitignored — `build:wasm` must run first or the build breaks).
- `npm run dev`, then drive the real app for each phase:
  - **Phase 1:** Playwright screenshots byte-identical to baseline, light and dark.
  - **Quality:** load "BOSL2 gear train", set Draft → triangle count drops and ms
    falls; Fine → both rise. Reload → setting persists.
  - **Status bar:** press play on "Animated turbine" — numbers must not blink or
    reflow; Render/Stop must not move horizontally.
  - **ResizeObserver:** drag every splitter and toggle the console — the model must
    not stretch and the grid spacing must stay correct **without** resizing the OS
    window. This is the regression most likely to ship unnoticed.
  - **Dock:** drag a slider in Parameters and read Model's numbers change *without
    switching anything*. Load a param-less script → spine, not a 288px hole.
  - **Diagnostics:** click a warning → cursor lands on the right span. Click an
    echo line → nothing, and it must not look clickable.
  - **Integrity:** toggle Fast → `EXACT` → `FAST PREVIEW`; export anyway and
    confirm the file is still watertight (`App.tsx:1443-1462` re-renders exact).
  - **Dimensions:** toggle on → callouts appear, grid numerics suppress; `PNG` →
    callouts are in the file.
  - **Shortcuts:** ⌘↵ renders and does *not* insert a blank line; Escape still
    dismisses the pick highlight.
- Desktop: `cd desktop && npm run dev` — confirm the native menu still drives
  New/Open/Save/Export/Reset View after the command registry lands.
- Both engines: run the checks above with the OpenSCAD toggle on; degradations
  (no customizer, no provenance, 3D-only) must now be *visible*, not silent.

---

## 7. Decisions

Settled before handoff:

1. **Scope — all six phases, as a milestone track.** This is M8, not a PR. It gets
   a roadmap doc (see §5) and ships incrementally.
2. **Palette — phase 4 is real.** Hold the colors through phases 0–3 so the token
   extraction is provably neutral, then do a considered evolution of *both* dark
   and light. Light mode gets designed, not inherited. The rejected ideas in §3
   ("what I changed after critique") stay rejected — don't re-propose a second
   accent hue or a self-hosted webfont without new evidence.
3. **Signature — prototype first.** Spike the dimension callouts, look at them
   against the real ruler grid and axis triad, then decide. Don't build the
   measure tool until the callouts are approved.
4. **Responsive/mobile — out of scope.** No `@media` work. See the note in §4.

### Still genuinely unknown

- Whether the Model dock section earns its space. That's why §4.4 ships the free
  data (`area`, `vertexCount`, parts list) *before* any Rust work — if nobody
  opens it, don't plumb manifold status and shell count across the wasm boundary.
- Whether `Fit` beside the nav cube is enough to justify deleting I/F/T/R, or
  whether muscle memory demands a keyboard path (⌘1–⌘7 via the palette is the
  cheap hedge).
