# M9 — Give the UI a ceiling

## 1. Context

M8 (`46a3e3c`, track D) restructured the web playground and predicted the toolbar
would go "from ~20 controls to ~12"
(`docs/roadmap/track-d-ui-structure-design.md:301`). It shipped at **19
interactive controls — 20 when `quality === "custom"` exposes `$fn`**
(`web/src/App.tsx:2024-2318`). One milestone later there is again nowhere to put
the next feature, and `App.tsx` grew **1866 → 2685 lines**. The
`web/src/components/` directory and `commands.ts` registry that M8's own doc
named at `:328` were never built.

Open `web/e2e/__screenshots__/default-light.png`. **The committed, blessed
baseline shows the topbar wrapped onto two rows at 1280×800**, eating ~80px of
vertical chrome, with a five-control animation transport on screen for a script
that never reads `$t`. The app is broken at its own test resolution, the failure
is checked into the repo as the reference image, and review approved it — because
there is no `.actions {}` rule in `index.css` (only `.actions button` at `:125`),
so the row is a block box that silently reflows, and **no test counts anything**.

That is the whole diagnosis. M8's control budget was a sentence in a Markdown
file. A sentence is not a mechanism.

Three consequences compound it:

- **Every control is re-declared per surface.** "Render" exists five times —
  CodeMirror keymap (`App.tsx:680`), window keydown (`:851`), status-bar button
  (`:2613`), palette entry (`:1914`), `HelpSheet.SHORTCUTS` (`HelpSheet.tsx:17`).
  Nothing forces the copies to agree, and they have already drifted: `⌘N`/`⌘O`/
  `⌘E`/`⌘0`/`⌘⇧S` are live Tauri accelerators
  (`desktop/src-tauri/src/lib.rs:989,992,1001,1024`) documented nowhere.
- **The safety net is not connected.** 13 Playwright specs and 6 baselines exist
  and run only when a human remembers; `.github/workflows/ci.yml`'s `web` job
  runs vitest + build. No ESLint at all — despite a hand-written
  `// eslint-disable-next-line react-hooks/exhaustive-deps` at `App.tsx:876`.
- **Real defects are riding along.** A dropped binary `.stl` is UTF-8-decoded
  (`App.tsx:1352,1373`), U+FFFD-mangled, written to `localStorage`, and fed to a
  parser that succeeds on garbage. Light-mode `--accent` on `--panel` is
  **2.79:1**. There is not one `:focus-visible` rule in 1281 lines of CSS.

**Outcome wanted:** a control budget that is enforced by physics rather than by
prose, a registry that makes "a control" countable, the missing code-CAD
controls, and a quality floor that stops the app shipping defects its own
reference screenshot displays.

---

## 2. Thesis

> **Make the UI answer to a ceiling it cannot argue with.**

Two mechanisms, deliberately redundant, because M8 proves one is not enough:

1. **A control is a `CommandDef`, not a piece of JSX.** One registry; the topbar,
   popovers, help sheet, keymaps and palette are all filters over it. This makes
   controls *countable* — the set of rendered `data-cmd` attributes must equal
   the set of defs pinned to the topbar, so a control cannot exist without a
   definition and a definition cannot hide in a wrapper `<div>`.
2. **The app must work at 1024px, and be usable on a tablet.** This is the
   ceiling. A cardinality assertion is one character away from being edited; a
   viewport is not. If the topbar must not wrap at 1024px, the budget is enforced
   by layout, and every future feature negotiates with the screen rather than
   with a reviewer.

**This deliberately overturns M8's "responsive/mobile is explicitly out of
scope."** New evidence: the blessed baseline already fails at 1280px, so the
non-goal did not hold in practice — it just went unmeasured. Making width a
first-class constraint is what converts the budget from prose into a gate.

Everything else in this milestone is what we do with the space that buys.

### The four questions

- **(a) Where does the next feature go?** It declares a `group` and lands in that
  group's popover, or a dock section. §3.1 pre-assigns the six features already
  on deck, including the class M8's zone map had no home for — *collections*.
- **(b) What did we delete?** Nine topbar slots, `Frames` as a transport control,
  the hand-maintained `SHORTCUTS` array, the palette's display-only `shortcut`
  string, the window keydown if-chain, and `when: false` hiding.
- **(c) What does a first-timer see change?** `Examples ▾` is still the second
  thing on screen; the toolbar is one row instead of two; the dock now tells them
  what their code built; and on a tablet the app works at all.
- **(d) What does the veteran never relearn?** `Fast`, the engine toggle, and the
  entire status bar — Render/Stop, triangles, dims, volume, ms, integrity — do
  not move. M8's rejection (c) stands.

**OpenSCAD is an engine option, not a fidelity story.** Per the product
decision, M9 ships no Quito-vs-OpenSCAD comparison panel. "Trust" in this
milestone means *the render is honest about itself* — integrity state, and
nothing failing silently (§8.4).

---

## 3. Zone map

| Zone | Holds | Why |
| --- | --- | --- |
| **Topbar** `.actions` (`App.tsx:2024-2318`) | `Examples ▾` · `Project ▾` · `Display ▾` · `Quality: Normal ▾` · `Quito⎪OpenSCAD` · `Fast` · `Export STL ▾` · `⌘K` · `? ▾` — **9 pinned defs, ≤12 hit targets, identical on web and desktop** | Admission rule, asserted in CI: a def is pinned only if it is a **comparison gesture flipped while looking at the model** (engine, Fast — M8's rule, unchanged), the **terminal action** (Export), or a group trigger. Triggers carry their live value (`Quality: Normal ▾`, `Export STL ▾`) so the row stays a readout. `Examples ▾` stays pinned: for a playground reached by share link, burying the word "Examples" is the worst available regression. |
| **Group popovers** | `project` (New, Import…, Share, .scad ⎪ desktop Open, Save, Save As) · `display` (today's 6 toggles + section axis/position) · `quality` (Draft/Normal/Fine + `$fn`/`$fa`/`$fs`) · `output` (format list, PNG, Frames zip) · `app` (help, shortcuts, **theme**, GitHub, version) | One `GroupPopover` over the existing `Popover.tsx`, rendering `commands(group, ctx)`. Generation fixes a live bug for free: the Display trigger's `active` is hand-written (`App.tsx:2072`) and omits `showDims` and `sectionOn` — so turning on the section plane, which *hides geometry*, leaves the trigger dark. |
| **Editor column** | Tab strip + CodeMirror. Gains keyboard-operable tabs and a `! # % *` modifier gutter. | Unchanged in role. |
| **Viewport** | Canvas, nav cube, `⤢ Fit`, ISO callouts, section clipping. **No new chrome over the canvas.** | M8 rejections (a) floating rail and (b) strip *above* the canvas both stand. |
| **Transport** — a strip **below** the canvas, inside `.viewer` | ▶/⏸ · `$t` scrub · readout · FPS · Steps | Only 2 of 10 examples read `$t` (`examples.ts:109,237`), so this is **collapsed to a 24px bar by default and expands when the main file reads `$t`** — it does not tax every script with 5 controls. M8's rejection was a strip *above*, which "steals the only axis the viewport has left (height) **and** sits over the nav cube." Below clears the cube; the height cost is paid honestly by collapsing. Ergonomics: `.anim input[type=range]` is `width:90px` (`index.css:886`) in a ~680px cell. |
| **Right dock** | `Parameters` · **`Objects`** (new) · `Model`. Sections, not tabs. | `Dock.tsx:1-4`'s rule holds — you read Model *while* dragging a Parameter. A tree's expensive axis is row count, which is vertical; the dock is full-height, the drawer defaults to 160px. |
| **Bottom drawer** | `Console`, with the severity chips moved inside its body. | The only wide-and-short container. Kept single-purpose in M9. |
| **Status bar** | Unchanged. | M8 rejection (c) stands — the core loop is "drag a slider, read the volume." |

### 3.1 The refill test — where the next six features go

The zone map is only real if the queued work already has homes. Pre-assigned now
so they are not invented into `Project ▾` later:

| Feature | Zone | Rows added |
| --- | --- | --- |
| Theme (Auto/Light/Dark) | `app` popover | 1 (ships in M9) |
| Measure tool | Objects section — selection state | 0 (a mode of an existing panel) |
| `!` / `*` modifier UI | Editor gutter + Objects row actions | 0 |
| Library sources | **`Resources` dock section** | new section |
| Font picker | `Resources` dock section | 1 row |
| Imported asset store | `Resources` dock section | 1 row |

The last three are the class M8's map had no container for: **collections, not
verbs.** "Add a library source" is add/remove/reorder/status — a managed list,
not a menu item. Declaring `Resources` now is what stops `Project ▾` becoming a
12-row menu by M11. Dock section budget: **4**. A fifth needs a new rule.

### 3.2 Target layout at 1440px

```
┌────────────────────────────────────────────────────────────────────────────────────────────────┐
│ QUITO  Examples▾  Project▾ │ Display▾  Quality: Normal▾  Quito⎪OpenSCAD  Fast │ Export STL▾ │ ⌘K ?▾ │
├────────────────────────┬───────────────────────────────────────┬───────────────────────────────┤
│ main.scad ● helpers  + │                              ┌────┐   │ ▾ PARAMETERS              ⟲   │
│────────────────────────│         ╱▔▔▔▔▔▔▔╲     ⤢ Fit  │cube│   │    width   ━━●━━     40       │
│  1  module lid() {     │        │  model  │           └────┘   │    lid     [x]                │
│ !2    cylinder(r=8);   │         ╲▁▁▁▁▁▁▁╱                     │───────────────────────────────│
│  3  }                  │    ├────── 40.00 ──────┤              │ ▾ OBJECTS                     │
│  4  difference() {     │          ⟨lid⟩ ⌀16.00                 │   ▾ difference()      12 480  │
│ *5    cutaway();       │    ╌╌╌╌ adaptive ruler grid ╌╌╌╌      │     ▸ lid()   ◉ iso    3 210  │ ← isolated
│  6  }                  │                                       │     ▸ shell()          9 270  │
│                        ├───────────────────────────────────────│───────────────────────────────│
│                        │ ▶ ══●════════ $t 0.340  15fps 20 steps│ ▾ MODEL              EXACT    │
├────────────────────────┴───────────────────────────────────────┴───────────────────────────────┤
│ ▸ Console 2      All · Errors 0 · Warnings 2 · Echo 3                                    ⌃ ⌄   │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ▶ Render │ 12 480 triangles · 40×20×12 mm · vol 4 812.30 · 0.8 ms │ EXACT │ console 2 │ quito 0.7.1 │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Breakpoints

Four, driven by what has to survive. The core loop — edit, render, read the
numbers — must work at every width.

| Width | Layout |
| --- | --- |
| **≥1280** | As above: editor ⎪ viewer ⎪ dock, three resizable columns. |
| **1024–1279** | Dock becomes an **overlay** sliding over the viewer (its splitter hides; `Objects`/`Parameters` reachable from the spine). Editor and viewer keep the splitter. Topbar single row — this is the enforcing width. |
| **768–1023** (tablet) | Editor and viewer become a two-pane **segmented switch** (`Code ⎪ Model`) filling the width; dock and console are overlays; the transport stays under the viewer; status bar drops `area` and the version. |
| **<768** | Single pane + segmented switch. Topbar collapses to `☰` (all groups) · `Quality ▾` · `Export ▾` · `⌘K`. Read-and-tweak, not authoring — the customizer and the viewer are the point at this width. |

Touch targets ≥44px below 1024. `.topbar`/`.statusbar` keep `nowrap` but stop
relying on it to hide overflow.

---

## 4. What moves, what dies

### Moves — zero capability loss

| From | To | Note |
| --- | --- | --- |
| `New` (`:2025`), `Share` (`:2053-2060`), `.scad` (`:2061-2068`), desktop `Open…`/`Save` (`:2044-2052`) | `Project ▾` | Topbar stops branching on `TAURI`. ⌘S/⌘⇧S and the native menu unchanged. **`Import…` is new capability, not a move** — web import is drag-only today (`App.tsx:1352-1400`); it needs an `<input type=file>` and is costed as such. |
| `Quality` select (`:2155-2168`) + conditional `$fn` (`:2169-2190`) | `Quality: Normal ▾` | Trigger keeps the live preset in its label, so the crash banner's "lower `$fn`" (`:2364`) still points at something readable. **Gain:** `$fa`/`$fs` reach a UI. |
| Animation cluster (`:2197-2257`) | Transport strip | Every control verbatim; the interval driver (`:908-920`) and `timeRef`/`stepRef` untouched, only re-parented. Scrub travel 90px → ~680px. |
| `Frames` (`:2258-2264`) | `Export ▾ → Frames (zip)` | It is an output, not a transport control. |
| `PNG` (`:2191-2196`) + format select (`:2273-2286`) | `Export STL ▾` split button | Primary click keeps the one-click auto-format contract. **PNG and .scad in the caret menu fire immediately and never write `exportFmt`** — `onResult` rewrites that state every render against `FORMATS_2D`/`FORMATS_3D` (`:1526-1531`), so a `"png"` value would be clobbered mid-session. Caret stays enabled at `triangleCount === 0`; `onSavePng` has no such guard and screenshotting an error state must keep working. |
| GitHub icon (`:2304-2317`) | `? ▾` (`group: "app"`) | An `openExternal` link with no in-app consequence. |
| Console severity chips (`:2547-2567`) | Inside the Console body | Drawer chrome stays panel-agnostic or it becomes the topbar one level down. |

### Dies

- **`Command.shortcut?: string`** (`CommandPalette.tsx:9-10`) — a hand-typed
  display string that can disagree with the binding. Formatted from `KeySpec`.
- **`HelpSheet.SHORTCUTS`** (`HelpSheet.tsx:15-23`) — generated. Strict gain: it
  omits `⌘N`/`⌘O`/`⌘E`/`⌘0`/`⌘⇧S` today. The hand-written `TIPS` array **stays** —
  4 of its 7 rows document capabilities that have no command (nav cube, `$vp*`,
  BOSL2 auto-fetch, modifier chars) and naive generation would delete them.
- **The window keydown if-chain** (`:836-858`) and the **CodeMirror keymap
  literals** (`:677-710`) — generated. `Prec.highest` on Mod-Enter is preserved
  (it must beat `basicSetup`'s `insertBlankLine`).
- **`when: false` hiding** (`CommandPalette.tsx:32`) — replaced by
  `disabledReason`, `aria-disabled`, and skip-on-arrow-walk. Hiding is the only
  thing here that can actually lose a capability: ⌘K → "frames" must find the
  row, greyed, reading *"needs a script that reads `$t`"*.
- **`prefs.paramsOpen`/`modelOpen`** as separate booleans → one `sections: string[]`.
- **`.stl` in `TEXT_IMPORT`** (`:1352`) → content sniff (`solid` prefix /
  `84 + 50n === size`). Sniffing rather than deleting the extension preserves
  ASCII-STL drop, which extension-based `import()` dispatch would break on rename.

### Considered and cut

- **A Quito-vs-OpenSCAD comparison panel.** Cut by product decision: OpenSCAD is
  an extra engine option, not a fidelity claim. (It would also have needed Rust —
  `shells`/`2-manifold`/`centroid` exist only in `xtask/src/main.rs:737,768`, not
  in `quito-geom`; `measure()` in `openscadGeometry.ts:285-320` returns volume and
  area only.)
- **Deleting the `Quito⎪OpenSCAD` toggle.** It stays pinned: with no comparison
  panel, the toggle *is* the feature.
- **A store migration.** §7.

---

## 5. Essential additions, ranked

| # | What | User problem | Cost | Zone |
| --- | --- | --- | --- | --- |
| 1 | **`web/src/renderState.ts`** — extract `onResult` (`App.tsx:1429-1581`) | Named at M8-design-doc:325 as "the right first extraction", never done. Without it every later split threads 20 props. | M | — |
| 2 | **`web/src/commands/` + `keys.ts` + `usePref` hook** | Five unsynced keybinding homes; a topbar nobody can count. `usePref` is load-bearing — see §7. | M | all |
| 3 | **Registry projections** — topbar, 5 `GroupPopover`s, HelpSheet, keymaps, palette | Where the 10 freed slots go. | M | topbar/popovers |
| 4 | **Responsive layout** — 4 breakpoints, overlay dock/console, `Code⎪Model` switch | The ceiling that makes #3 stick, and the app is unusable on a tablet today. Requires the `.viewer` flex refactor (§7). | L | all |
| 5 | **Theme toggle (Auto/Light/Dark)** | M8 item #14, never built; `prefers-color-scheme` only. Needs `data-theme` written from a pref instead of `currentMode()` (`App.tsx:102,892-901`). | XS | `app` |
| 6 | **`$fa`/`$fs` in `Quality ▾`** | Persisted (`prefs.ts:30-31`), validated (`:125-126`), injected (`:94-95`), no UI — explicit TODO at `App.tsx:460-462`. The Quality tooltip already *promises* them (`:2162`). Tolerance tessellation is how you match OpenSCAD's 12°/2mm defaults. | XS | `quality` |
| 7 | **`persist` as a registry field, via `usePref`** | Ortho never calls `savePrefs` (`:1211-1214`, unlike its four siblings at `:1189-1209`); console open + filter are bare `useState` (`:418-421`). Three defects, one missing invariant. | XS | — |
| 8 | **Palette: disable-with-a-reason**, and register `play`, `seek`, `frames`, `download-scad` **before** their buttons move | Otherwise "where did my ▶ go" has no answer anywhere. None of those four are in the 20-command registry today. | XS | palette |
| 9 | **Transport strip** with `$t`-driven expand/collapse | 5 controls tax every script; the scrubber is 90px. | M | viewport |
| 10 | **Objects section** — a Lezer outline of `files[0]`, with triangle counts joined on by span | No representation of the object. §7 specifies which of the two possible trees this is. | M | dock |
| 11 | **Isolate + retargeted ISO callouts** (the signature, §6) | You can inspect a sub-assembly only by commenting out code. | M | viewport + dock |
| — | ***below here is M10*** | | | |
| 12 | Measure tool (point-to-point, same drafting callout) | | M | viewport |
| 13 | Hover docs + go-to-definition | Needs nothing from `quito-lsp`: `builtins.ts`, `signature.ts`, `complete.ts` already have the data. | S | editor |
| 14 | `Resources` dock section (library sources, fonts) | The collection container declared in §3.1. | M | dock |
| 15 | Examples browser with thumbnails | `examples.ts` is 1050 LOC of inline strings behind a `<select>`. | M | `Examples ▾` |

---

## 6. The signature move

**Isolate and dimension any part of your model, without touching the code.**

Click a row in `Objects` (or a face in the viewport): every mesh leaf not under
that node's provenance span is hidden, and M8's ISO callouts **retarget to that
node's bounding box**, gaining one new drafting element — a circled leader
balloon labelled with the module name (`⟨lid⟩`). Press Escape to un-isolate.

Why this and not a properties panel:

- **It composes with M8 rather than repeating it.** Same world-space geometry in
  `this.scene`, same `makeTickLabel` sprites (`viewer.ts:534`), same grid-label
  suppression (`:484,500,512`), same amber palette. `rebuildDims` takes an
  optional `Box3`.
- **It travels.** `capturePng` renders `this.scene` through `this.renderer` and
  `toBlob`s that one canvas (`viewer.ts:1454-1462`); the nav cube is a *separate*
  canvas (`:1084-1106`). So a screenshot of an isolated part is a **detail
  drawing of a sub-assembly, produced from code, with no CAD app** — and no DOM
  chrome leaks in.
- **It is instant and non-destructive.** Viewport-only: no re-render, no source
  edit, no `!` written into the user's file. The engine's `!`/`*` modifiers stay
  a separate M10 feature precisely because writing to the buffer changes undo,
  dirty state, and share-link contents.
- **It costs nothing new on the wire.** Every render already ships, per leaf, the
  outermost→innermost stack of source byte ranges
  (`crates/quito-wasm/src/lib.rs:63-67`). `highlightSpan` already matches **any**
  span in the stack (`viewer.ts:751-753`); only picking takes `spans[last]` (`:726`).

Three constraints that make it a feature rather than a strobe:

- **Report triangles and bbox for an isolated subset, never volume.** A subset of
  leaves is not a closed solid; its volume is meaningless. The Model section
  shows the whole-model volume with the isolated part's tri count and extent.
- **Transient vs committed selection.** `highlightFromCursor` fires on every
  arrow key (`App.tsx:728-729`). Cursor and hover give *transient* selection (the
  existing cyan wash only); a click or an Objects row gives *committed* selection
  (isolate + retargeted callouts). Escape clears, reusing `highlightDismissedRef`.
- **Selection lives in the viewer, not in React.** `rebuildDims()` is called from
  `setMesh` (`viewer.ts:627`), `setTheme` (`:283`) and `setDimensionsVisible`
  (`:912`) — ~15×/s during playback. Store `this.selectionSpan`, re-resolve it
  from `provGroups` after every `setProvenance`, and clear it silently when the
  span no longer matches. Node labels slice the **rendered-source snapshot** kept
  by `renderState.ts`, never the live buffer, or every label is wrong for the
  150ms debounce + render window.

Demo sentence: *"Click the lid. The lid is isolated, measured, and in the PNG."*

---

## 7. Architecture

**Keep the ref-shadow pattern. Do not migrate to a store.** The mount effect has
`[]` deps (`App.tsx:503-877`) and the file holds **45 `useState` and 43 `useRef`**
— M8's "22 state / ~40 refs" (design-doc:332) was accurate at 1866 lines and is
now about half the real surface. A store migration in the same milestone that
moves ten controls, with Playwright not yet in CI, is how you ship a stale-closure
regression nobody finds. **Write the actual rule down** — the one M8's "Preserve"
paragraph described by symptom: *nothing reachable from `onResult` may read a
`useState` value.* `cb` at `App.tsx:569` captures the mount render's binding
forever; it is safe today only because it reads zero state variables.

### The three pieces that must land together

**1. `renderState.ts` — split pure from imperative.** `onResult` is ~9 state
writes and ~18 side effects; a pure reducer cannot hold
`markRenderPending`/`applyDiagnostics`/`viewer.setMesh`/`applyScriptCamera`.
Specify it as:

- `reduce(prev: RenderState, r: RenderResponse): RenderState` over the **9 pure
  fields only** — `status`, `schema`, `overrides`, `is2D`, `exportFmt`,
  `version`, `diagCounts`, `renderRev`, `renderedSource`. No React import; assert
  that in a test.
- `applyRenderEffects(r, deps)` stays in `App.tsx`, taking a deps object of
  **refs only**. It keeps every side-effect contract: arms/settles the crash
  sentinel (`:1440`, `:1576`) and bumps `data-render-rev` (`:1580`), the e2e
  completion signal.
- **This forces a state consolidation the reducer shape implies:** those 9
  `useState`s become one, called as `setRenderState(prev => reduce(prev, r))`.
  Say so up front — the call site is inside the `[]`-deps effect and cannot read
  `state` any other way.

**2. `CommandDef`, with widgets and a `Ctx` cap.**

```ts
// web/src/commands/types.ts — pure data, module level, no hooks, no JSX
export interface CommandDef {
  id: string; group?: Group; pin?: "topbar" | "transport";
  title: string | ((c: Ctx) => string);      // engine/Fast tooltips are 4-branch today
  help?: string; key?: KeySpec; menu?: string;
  persist?: { key: keyof Prefs; on: "change" | "commit" };  // "commit" default for range
  widget?: WidgetSpec;                        // toggle | segmented | number | range | select
  when?: (c: Ctx) => boolean; disabledReason?: (c: Ctx) => string;
  value?: (c: Ctx) => unknown; run(c: Ctx, api: Api): void;
}
```

`widget` is the unglamorous part that makes the count hold: roughly half this
app's chrome is *values*, not verbs (`$t`, FPS, Steps, `$fn`, `$fa`, `$fs`,
section axis/position, quality, export format, theme). A verbs-only registry
leaves them hand-written in JSX — the exact mechanism by which the row refilled.
**Write `Ctx` and `Api` out in full before any def file, and cap `Ctx` in
`commands.test.ts`** (`Object.keys(ctx).length <= N`); a 30-field prop bag is the
review-time failure.

`KeySpec` needs **three** axes, not one: `scope: "editor" | "global" | "both"`,
`prec: "high" | "default"` (Mod-Enter must beat `insertBlankLine`), and
`owner: "web" | "native" | "both"`. Without `owner`, desktop double-fires every
`⌘N`/`⌘O`/`⌘E`/`⌘0` — native accelerator *and* window handler — and the web help
sheet advertises `⌘N`, which is browser-reserved and cannot be prevented.

**3. `usePref` — the milestone's silent-failure guard.** Four prefs are
*tri-written* today: `linkHighlight` (`:1114`), `fastPreview` (`:1125`),
`quality` (`:1135`), `engineKind` (`:1221`) each set a shadow ref **and** state
**and** `savePrefs`. The `[]`-deps `renderNow` reads `fastPreviewRef.current`
(`:634,649`) and `qualityOverrides(qualityRef.current)` (`:600`); `buildEngine`
reads `engineKindRef.current` (`:581`). A generated toggle with
`persist:"fastPreview"` that only calls `setFastPreview` + `savePrefs` produces:
**the button lights amber, the pref survives reload, and every render is still
exact** — invisible in the UI and invisible in the screenshot baselines, because
the canvas is masked. Make `usePref<K>(key)` own state + shadow ref + `savePrefs`
atomically, the only way to declare a persisted value, with a registry invariant
that every `persist` def resolves through it.

Plus **two refs**, assigned during render alongside the four that already do this
(`App.tsx:1905-1908`), with `ctxRef.current ??= buildCtx()` so the mount path can
never see null: `ctxRef` (the `[]`-deps keydown handler and the Tauri menu relay
have no live `ctx`) and `actionsRef` (identity-stable `Api`).

### Files

| File | What |
| --- | --- |
| `web/src/renderState.ts` | `reduce` + the rendered-source snapshot §6 needs. |
| `web/src/commands/{types,keys,index}.ts` + one file per group | ~50 defs. `keys.ts` is the single formatter: `KeySpec` → CM binding, window matcher, `"⌘⇧F"` display string, Tauri accelerator. |
| `web/src/commands/commands.test.ts` | Invariants: rendered `[data-cmd]` set **equals** `defs.filter(pin==="topbar")`; no duplicate `(key, scope, prec)`; no id produces both a CM binding and an unguarded window matcher; `menu`-tagged ids equal the 7 in `build_menu` (`desktop/src-tauri/src/lib.rs:972`) — otherwise Phase 2 creates a *fifth* source of truth instead of killing the fourth. |
| `web/src/components/GroupPopover.tsx` | Wraps `Popover.tsx`; adds **close-on-action** (today it closes only on outside pointerdown or Escape — right for Display's toggles, wrong for `Project ▾`). |
| `web/src/components/Transport.tsx` | The 5 controls, in the `.viewer` column. |
| `web/src/viewer.ts` + `index.css` | **The transport is not a JSX move.** `.viewer` is `position:relative` with `canvas{width:100%;height:100%}` (`index.css:573-583`); the nav cube is appended to `canvas.parentElement` and the `ResizeObserver` observes it (`viewer.ts:208,215-216`). Wrap the canvas in `.viewer-canvas` (flex child, `min-height:0`), make `.viewer` `display:flex;flex-direction:column`, and move both the cube parent and the observer target to the wrapper — otherwise the canvas overlaps the strip, and expanding the strip resizes the canvas without firing the observer (stretched view, desynced `pickAt` math). |
| `web/src/objectTree.ts` (pure) + `components/ObjectsSection.tsx` | **Parse `files[0]` with the existing Lezer parser for the shape, and join triangle counts on by span.** Not geometry-derived: `partition_provenance` (`crates/quito-geom/src/lib.rs:558-700`) emits one leaf per *surviving* region, so a `difference()`'s tool operands produce no leaf (`:626-660`), `hull`/`minkowski`/`linear_extrude` collapse their whole subtree (`:697-703`), and a `for` loop's N instances share one span stack. Rows with no matching span render "no geometry" instead of vanishing. Also: `stmt_span` returns `None` unless `in_main` (`crates/quito-eval/src/lib.rs:518-524`), so this is the user's own file — unattributable leaves group under one `(library geometry)` row. Memoize on the parsed array's identity and gate on the section being open; `onResult` fires ~15×/s during playback. |
| `web/vitest.config.ts` | Second project: `environment:"jsdom"`, `include:["src/**/*.test.tsx"]`. `.tsx` cannot match the glob today. |

**Net on `App.tsx`:** −150 (`onResult`) −88 (command array) −~300 (topbar/transport
JSX) −~30 (keydown chain) ≈ **2685 → ~2100**, with the state block and mount
effect deliberately intact. If `renderState.ts` and `usePref` do not land in the
*same* milestone as the registry, M9 ends with 2685 lines spread over more files.

---

## 8. Quality floor

Landing with the phase that touches each surface. Every number below was computed,
not estimated.

**8.1 Contrast — first, because `:focus-visible` depends on it.** Light `--accent`
on `--panel` is **2.79:1**, so an amber focus ring is invisible in light mode.
Fix light `--accent` → ≈`#a45a06` (4.55:1) or add `--accent-text` for text/border
roles; light `--muted` 4.36:1 → ≈`#5b6472`; light `--warn` 3.19:1; `--border` is
**1.24:1 in both themes**, failing 1.4.11's 3:1 for control boundaries — add
`--border-strong`. `.tab-close` at `opacity:.6` is **2.22:1** in light on an
enabled destructive control. In dark mode `.viewer-fit`'s fill is byte-identical
to the viewport clear color, separated only by a 1.24:1 border.

**8.2 Keyboard and AT.**
- `:focus-visible` exists at all — zero focus rules in 1281 CSS lines; the only
  focus-adjacent rule is `outline:none` at `:316`.
- Dialogs behave like dialogs: `aria-modal="true"`, focus in on open, trap,
  restore on close, Escape from anywhere (today only from the input,
  `CommandPalette.tsx:78`). Palette gets `role="combobox"` +
  `aria-activedescendant`, list gets `role="listbox"`/`option` + `aria-selected`
  (`:80-99` is a bare `ul`/`li`).
- Fix `Popover` **once**, before M9 quadruples it: it claims
  `aria-haspopup="menu"` (`:42`) with no `role="menu"`, no arrow keys, no
  focus-in, no restore.
- `aria-pressed` on every toggle (engine, Fast, section axes as a real
  radiogroup, console chips); `aria-controls` on `.dock-section-head` (it already
  has `aria-expanded`); both on `.console-toggle` (`App.tsx:2663-2669`).
- Editor tabs are keyboard-dead: `<div className="tab" onClick>` with no
  `tabIndex` (`:2421-2424`) — you can Tab to *delete* a file but not to switch to
  one. `role="tablist"` + roving tabIndex + F2 rename.
- `ResizeHandle` (`:38-45`): `tabIndex={0}`, `aria-valuenow/min/max`,
  Arrow/Home/End. Three splitters are mouse-only, so every persisted size is a
  mouse-only setting.
- `prefers-reduced-motion`: zero occurrences in `web/`. Needs the CSS query
  **and** a `matchMedia` read in `animateToDir` (`viewer.ts:1344`, `dur:350`) —
  CSS cannot reach a rAF-driven camera.
- Add `<main>`, an `<h1 class="sr-only">`, and `aria-live` on the status bar; a
  screen-reader user is never told a render finished, failed, or came back
  DEGRADED.
- Selection cyan vs mesh amber is **1.01:1** — a pure hue difference. Not a
  second accent hue (out of bounds): lighten selection to ≈`#b3e5fc` for a value
  delta and add a non-color cue (thickened edge overlay on the isolated leaf).
  Light-mode mesh amber vs viewer background is **1.83:1**.

**8.3 Bugs to fix in Phase 0, independent of the refactor.**
- **`.stl` content sniff** — live data corruption (`App.tsx:1352,1373`).
- **`Display ▾` `active`** omits `showDims` and `sectionOn` (`:2072`); derive it
  from the group's defs so a new toggle cannot be forgotten.
- **`⌘S` in the browser is a swallowed no-op** — the CM keymap `preventDefault`s
  and `saveActive` returns immediately when `!TAURI` (`:690-708`, `:1020-1021`),
  suppressing the browser's own dialog with no replacement, and only when the
  editor has focus. Bind it to `onDownloadScad()` on web, in the window handler.
- **The ~9.6MB OpenSCAD fetch happens inside the render, under the shared 20s
  watchdog** (`engine.ts:16,195-200`), so a slow download aborts with *"the model
  may be too complex. Reduce $fn."* Fetch outside the render clock with a visible
  downloading state.
- **`#` is labelled `!`** in `Dock.tsx:139` and `tokens.ts:24`
  (`crates/quito-syntax/src/ast.rs:172-178` is unambiguous).

**8.4 Silent degradations that get a voice.** The OpenSCAD engine has no
customizer, no provenance, and is 3D-only (`openscadWorker.ts:15-22`) — the
Objects section must render an explainer and **never vanish**, since a section
that disappears on the button a user presses fifty times a session is the same
silence plus a layout shift. A missing library is dropped by the fetch layer
(`library.ts:102`) and warned about by the evaluator as *"Can't open include
file"* — correct but misattributed; say the fetch failed. Desktop silently falls
back to bundled wasm when no binary is found.

**8.5 Testing.** Append Playwright to the **existing** `web` job after
`npm run build` (a separate job re-pays ~5 min of `wasm-pack`), and point
`webServer` at `vite preview` — today it screenshots an unminified dev build with
HMR injected. Add `@axe-core/playwright` across 4 states × 2 themes and one
keyboard-only journey spec (it fails today at the editor tab strip, not at the
param sliders — those are native `<input type=range>` and already focusable).
jsdom vitest project table-testing `reduce` across ok/error/stopped/geomErrors/
preview/is2D/schema-change/multi-color. ESLint + `react-hooks` + `jsx-a11y`, and
`npm run format:check`, gated — `jsx-a11y` alone catches the tab strip and the
missing `aria-pressed`. **There is no e2e coverage of the engine toggle,
animation, export, or share today**; add engine-toggle coverage before Phase 2
touches `engineKindRef`.

**Spec migration:** write the *post-move* specs in the PR **before** the move,
against `data-cmd` selectors that survive it (`[data-cmd="quality-fine"]` works
both before and after). `quality.spec.ts` drives `.quality-select` with
`selectOption`, deleted by Phase 2; `palette.spec.ts:13` asserts exactly 1 result
for "console", which a ~50-def registry with disable-not-hide breaks;
`baseline`/`display`/`dimensions`/`help` all shift. Rewriting six specs in the
same PR as the move they guard is theatre.

---

## 9. Sequencing

Same shape as `docs/roadmap/track-d-ui-structure.md`. Lands as
`docs/roadmap/track-e-ui-ceiling.md` with a new row in `docs/roadmap/README.md`.
Each phase independently shippable. **The registry is what gets cut to M10 if the
milestone runs out** — cutting it costs nothing a user can see, which is why the
signature is not scheduled behind it.

| Phase | Contents | Gate | Changeset |
| --- | --- | --- | --- |
| **0 — Net, floor, and the four bugs** | Playwright appended to the `web` job (`vite preview`, Linux baselines re-blessed, diff PNGs uploaded); ESLint + prettier gated; jsdom vitest project; §8.3 bug fixes; §8.1 contrast; `:focus-visible`; `prefers-reduced-motion`. | Tab from the editor in light mode shows a ring ≥3:1 on `--panel`. A dropped binary STL is refused, not decoded. | `patch` (contrast + bugs are user-visible) |
| **1a — `renderState.ts`** | `reduce` + the 9-field state consolidation + `applyRenderEffects`. | 13 specs and 6 baselines green with **zero re-blessing**; `renderState.test.ts` covers 8 branches. | none |
| **1b — `usePref`, `ctxRef`, `actionsRef`** | Separate PR — shares nothing with 1a, and a combined red phase can't be bisected from a screenshot diff. | Toggle Fast → renders actually change (the shadow-ref invariant). | none |
| **2 — Signature spike → Objects + isolate + retargeted callouts** | Spike the retarget on a branch first, as M8 did for the callouts. Decision gate: ship / iterate / drop. Then `objectTree.ts`, the Objects section, viewer selection state. | `objectTree.test.ts` incl. the empty/collapsed/`for`-loop cases; no dock rebuild while the section is collapsed (call counter); tree build <4ms on the Parthenon. | `minor` |
| **3 — Registry, as a shadow first** | All ~50 defs + `keys.ts`; generate **only** the palette and HelpSheet (no layout consequence, existing coverage); add `data-cmd` to every hand-written topbar control; `commands.test.ts`. **Moves zero pixels.** | Rendered `[data-cmd]` set equals `defs.filter(pin==="topbar")`; no duplicate `(key,scope,prec)`. Fully revertible. | none |
| **4 — Projections, one group per PR** | `Project ▾` first (4 low-traffic controls, no shadow refs), `Quality ▾` **last** (it touches `qualityRef` and the render path). Plus `$fa`/`$fs`, `persist`, theme toggle, palette disable-with-reason + the 4 missing registrations, §8.2 dialog/toggle/tab a11y. | Topbar is 9 pinned defs / ≤12 targets and does not wrap at 1024px. | `minor` |
| **5 — Transport + Export split + `.viewer` flex refactor** | Includes the canvas-wrapper/ResizeObserver move. **Re-blesses all 6 baselines** — and note in the changeset that `capturePng`/`Frames` output dimensions change for every user. | Gear train animates from the strip; a non-`$t` script shows the 24px collapsed bar; `exportFmt` unchanged after a PNG export. | `minor` |
| **6 — Responsive** | 4 breakpoints, overlay dock/console, `Code⎪Model` switch, 44px touch targets. | The core loop completes at 1024, 820 and 480px. New baselines at each. | `minor` |

---

## 10. Verification

`npm run build:wasm && npm install && npm test && npm run build` first —
`web/engine/` is gitignored and the build breaks without it. Then `npm run dev`
and drive the real app. `data-render-rev` on the status bar is the completion
signal.

| Phase | Load | Touch | What must change |
| --- | --- | --- | --- |
| 0 | default, light theme | Tab from the editor; then drop a binary `.stl` | A visible amber ring on the first control. The STL is refused with the binary-not-supported message; no new tab; `localStorage` unchanged. |
| 1a | "Twisted vase" | ⌘↵, then break the syntax and ⌘↵ | `data-render-rev` increments; status bar red with the parse error; console auto-opens. Baselines unchanged. |
| 1b | default | Toggle `Fast`, then ⌘↵ | Triangle count and `ms` change **and** the integrity badge reads `FAST PREVIEW`. (The failure mode is the button lighting up while renders stay exact.) |
| 2 | "Parthenon" (polychrome) | Click a column in the viewport | Only that column renders; the ISO callouts retarget to its bbox with a `⟨module⟩` balloon; the Objects row highlights; the editor cursor jumps. `PNG` → the isolated part **and its callouts** are in the file. Escape restores. Switch to the OpenSCAD engine → Objects shows an explainer and does **not** vanish. |
| 3 | default | Nothing | Screenshots identical to Phase 2. Add a topbar control without a def → `commands.test.ts` red. |
| 4 | default | `Quality: Custom`, `$fa=2`, `$fs=0.1`, ⌘↵ | Triangle count **rises** vs `$fn`-only and survives reload. Toggle Orthographic, reload → still orthographic. Turn on the section plane → `Display ▾` lights amber. Set theme to Light while the OS is dark → stays light after reload. ⌘K → "frames" on a non-`$t` script → present, greyed, with its reason. |
| 5 | "BOSL2 gear train ($t)" (`examples.ts:237`) | Drag the strip's scrubber end to end | A full rotation; the scrubber is ~680px. Load the default project → the strip is a 24px bar. `Export ▾ → PNG`, then click primary Export → an **STL**, not a PNG. |
| 6 | default, window at 1024 / 820 / 480px | Edit → ⌘↵ → read the volume → drag a param | The topbar is one row at 1024. At 820 the `Code⎪Model` switch works and the dock overlays. At 480 the customizer is usable with 44px targets. No horizontal scrollbar at any width. |

Desktop (`cd desktop && npm run dev`): the native menu still drives
New/Open/Save/Export/Reset View after Phase 3; ⌘N/⌘O/⌘E fire **once**, not twice.

---

## 11. Rust follow-ups — out of scope, noted

Web-only per the product decision. These stay queued and blocked:

1. **Binary `Vec<u8>` asset channel.** `MapResolver::load_bytes`
   (`crates/quito-wasm/src/lib.rs:324`) re-encodes stored *text* only. Blocks
   browser `import()` of binary STL/3MF and `surface(png)` heightmaps. Needs an
   IndexedDB asset store too — a 20MB STL cannot live in `localStorage`. Its own
   milestone; the Phase-0 `.stl` sniff is unrelated and does not wait for it.
2. **Mesh integrity in `quito-geom`** — `is_manifold`, `components`, centroid,
   inward-facing. These exist only in `xtask/src/main.rs:737,768`. Would let the
   Model section report shells and manifoldness.
3. **Generated native menu.** `build_menu` hardcodes 7 relayed items
   (`desktop/src-tauri/src/lib.rs:972`); the menu bar reaches 7 of ~50 actions.
   Tauri v2 runtime `set_menu` plus a `predefined` variant in `KeySpec`, or macOS
   loses ⌘C/⌘V (`lib.rs:1013-1021`). Until then, `commands.test.ts` asserts the
   TS `menu` ids equal the Rust 7 so Phase 3 does not create a fifth source of truth.
4. **`asserts_run`** (`crates/quito-eval/src/lib.rs:234`) → `RenderResult`, for an
   "N assertions passed" chip.
5. **Engine-side welded exports** — the browser rebuilds OFF/OBJ/AMF from
   non-indexed triangle soup (`web/src/stl.ts`), ~3× larger than
   `Mesh::to_off`/`to_obj`/`to_amf`.
6. **Fuel-budget cancellation** — `eval_program_with_budget` exists; wasm uses a
   20s worker-kill.
7. **Engine-reported `$t`** — one bool would replace the Phase-5 main-file Lezer
   scan, which cannot see `$t` inside an included library.
