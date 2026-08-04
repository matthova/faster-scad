# Track D — Web UI structure & essential additions (M8)

**Goal:** give every existing playground feature one obvious home, make room for
the next one, ship a real design-token system, and add the handful of missing
controls a code-CAD tool cannot honestly ship without — all without a palette
change until the token extraction is provably neutral.

**Why after A/B:** the geometry is now trustworthy (track A) and the switcher
workflow works (track B). The remaining friction is structural: the playground
grew feature-by-feature and every control landed in one non-wrapping flex row.
`.actions` (`web/src/App.tsx`) holds ~20 controls spanning eight unrelated jobs;
`App.tsx` is ~1,866 lines and is the entire app; the "design system" is 8 color
tokens and nothing else; nothing is resizable; there are zero `@media` queries;
and the frontend has no ESLint, no component/DOM tests, and no Playwright. The
engine already produces data the UI throws away (`area`, `vertexCount`) and
supports controls the UI cannot reach (`$fn`/`$fa`/`$fs`) — the crash-recovery
banner tells the user to "lower `$fn`" with no control to do it.

**Scope:** all six phases, shipped incrementally, one changeset per shipped PR.
**Out of scope:** responsive/mobile — no `@media` work.

---

## Sequencing

The constraint that drives the order: **zero UI safety net.** Every regression
is currently caught by a human squinting. Build the net first, extract tokens
under it, then move things.

### Phase 0 — Safety net

Playwright + a handful of screenshot baselines (default project, an example,
error state, light + dark). Prettier. Baselines green before anything moves.

### Phase 1 — Tokens (palette unchanged)

Spacing / radius / type / elevation / z-index scales; `ui-monospace` mono stack
→ one token; pull `viewer.ts`'s ~9 hardcoded hexes into the shared token source
(several only take effect via `rebuildCubeFaces()`/`buildGrid(force)`, so this
must precede any palette work); fix `index.css`'s raw `#23272f`. Palette
unchanged — screenshots byte-identical. That neutrality is the whole point of
doing tokens first.

### Phase 2 — Free wins, no layout change

- **Render quality** `$fn`/`$fa`/`$fs` — Draft / Normal / Fine / Custom,
  injected as `-D` literals like the customizer does. Persist in `prefs.ts`. Not
  in the share link.
- **Status-bar stability** — hold last-good values across renders, reserve
  column widths, tabular numerals. Prerequisite for adding anything to that bar.
- **`ResizeObserver` on the canvas container** — hard prerequisite for resizable
  panels; `Viewer` listens only to `window.resize` today.
- **Free inspector data** — surface `area` and `vertexCount` (already on the
  wire) plus a per-colour parts list from the `groups` JSON.
- **Clickable diagnostics** — scoped to the structured `diagnostics` array only.
- **Render-integrity state** — Exact / Fast preview / Degraded, one word in the
  status bar.
- Desktop `engine_version` fix (the Tauri command exists but is unused).

### Phase 3 — Structure

Topbar split (source-out left, model-out right); `Display ▾` and `Quality ▾`
popovers; delete I/F/T/R + `Reset view` (the nav cube already does all 7 presets
with fly-to); add `Fit`; right dock as collapsible sections (`Parameters`, then
`Model`) with a ~28px spine when there are no params; resizable console drawer
with severity filter chips; resizable + persisted panel sizes. Needs 0 and 2.

### Phase 4 — Palette & light mode

A considered evolution of both dark and light palettes; three-elevation values
in both; editor *chrome* aligned to tokens (syntax colors untouched). Only after
1 and 3 land, so the token extraction is provably neutral first.

### Phase 5 — Signature (spike first)

Prototype dimension callouts (real ISO dimension annotation in world space) on a
branch and look at it against the ruler grid and axis triad before committing.
Decision gate: ship / iterate / drop. Measure tool only if the callouts land.

### Phase 6 — Capability

Section/clipping plane; local file import + a `Vec<u8>` channel for binary
STL/3MF and PNG heightmaps; examples browser (JSON, thumbnails); silent-
degradation surfaces; help/shortcut sheet + first-run. Each its own PR.

---

## Changesets

One per shipped PR, not one for the track. Phases 0 and 1 are `docs`/`refactor`-
shaped and need none (byte-identical screenshots ⇒ not user-facing). Phases 2,
3, 5, 6 each need a `minor` — new capability.

## Preserve

The ref-shadow pattern is load-bearing: the mount effect in `App.tsx` has `[]`
deps and its closures must read live values (~40 refs mirror 22 state values).
Any component split must keep live-value access or move to a real store — don't
half-migrate.

## Exit criterion

A new user can find every feature without a tooltip hunt; every control has one
obvious home; nothing in the app fails silently; and the topbar has room for the
next feature.

---

_The full design writeup — verified feature inventory, rejected placements,
visual direction and the self-critique behind it, and the ranked list of all 15
essential additions — lives alongside this track and drove the phasing above._
