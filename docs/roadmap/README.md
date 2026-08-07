# Quito roadmap — post-M5 tracks of work

_Status snapshot: 2026-07-27. M0–M5 are complete: full language, full geometry
surface, ~25× geomean vs OpenSCAD/CGAL (bytecode VM + geometry cache),
customizer, multi-file projects, import/export of STL/OFF/OBJ/3MF/AMF/DXF/SVG,
PWA, `$t` animation playback. CI is green and the playground is live at
<a href="https://matthova.github.io/faster-scad/playground" target="_blank" rel="noopener noreferrer">https://matthova.github.io/faster-scad/playground</a>._

This directory breaks the candidate next milestones into three tracks, each
with a detailed writeup. Every claim below was verified against the code at
commit `34ee83a` (file/line references are to that state).

The sub-day "cherry-pick" bundle (playground link, doc staleness, CI branch
filter, and the clippy/fmt/tsc CI gates) has since landed and been removed from
this directory.

| track | doc | theme | effort | exit criterion (summary) |
|---|---|---|---|---|
| **A (recommended M6)** | [track-a-trustworthy-geometry.md](track-a-trustworthy-geometry.md) | Fix silent-wrong-geometry bugs; build the geometry oracle | ~3–4 weeks | `xtask geom` 60/60 golden cases vs OpenSCAD 2024.12 + `xtask bosl2` 16/16 with assertion checking, both in CI |
| **B (proposed M7)** | [track-b-switcher-experience.md](track-b-switcher-experience.md) | Product parity for daily use: desktop Save, inline diagnostics, color/#/% rendering, PNG export | ~3 weeks | An OpenSCAD user can run a real project on Quito desktop without a workflow wall |
| **C (fold into A/B)** | [track-c-ci-hardening.md](track-c-ci-hardening.md) | Make every headline claim machine-enforced on every PR | ~1.5–2 weeks | No claim in the README that CI doesn't check |
| **D (M8)** | [track-d-ui-structure.md](track-d-ui-structure.md) | Web UI structure: token system, re-homed controls, and the missing code-CAD controls (`$fn`, quality, inspector, integrity) | six phases, incremental | Every feature has one obvious home; nothing fails silently; the topbar has room for the next feature |
| **E (M9)** | [track-e-ui-ceiling.md](track-e-ui-ceiling.md) | Give the UI a ceiling: a `CommandDef` registry that makes controls countable, a responsive layout enforced at 1024px, the `renderState.ts`/`usePref` refactors, the Objects/isolate signature move, and a quality floor with Playwright in CI | seven phases (0–6), incremental | The topbar is ≤12 hit targets and does not wrap at 1024px; a control cannot exist without a def; the app is usable on a tablet; the quality net runs on every PR |

## Why this ordering

The single organizing observation from the repo survey: **the remaining risk is
silent wrong answers, not missing features.** Several very common constructs —
concave `offset()`, non-convex `minkowski()`, `projection()` inside 2D booleans —
currently produce **wrong geometry with no warning**, and the only oracle harness
(echo, 24 cases) covers none of the geometry surface. (The transform-binding bug
— `rotate(45, [1,1,0])` ignoring its axis, `translate(v=…)` binding nothing — is
now fixed; see track A, A1.) For a switcher, one silently-wrong print is fatal to
trust in a way a missing feature never is.

Track A follows the project's own milestone discipline (M1's exit was "echo
oracle diffs pass"; M6's should be "geometry oracle diffs pass"), subsumes the
most valuable half of track C as a byproduct (BOSL2-in-CI, geometry regression
gates), and de-risks track B: polishing the adoption funnel into a tool that
silently mis-rotates parts would convert adopters into detractors.

## Constraints that apply to all tracks

- **Clean-room policy** (CONTRIBUTING.md): never read OpenSCAD source. All
  compat work is black-box — user manual, Wikibook, and observed behavior of
  the `openscad` binary (echo output, exported meshes). Oracle testing is
  inherently clean-room-safe: it compares outputs, not implementations.
- **Dual kernels**: every geometry change must work on both the native C++
  Manifold backend and the pure-Rust `boolmesh` wasm backend (`Kernel` trait,
  `crates/quito-geom/src/lib.rs`), and keep the existing differential test
  passing.
- **The tree-walk interpreter is the reference semantics**; the bytecode VM
  must never change results, only timing.
- **Oracle machine**: OpenSCAD 2024.12 lives at `/opt/homebrew/bin/openscad`
  on the dev machine. CI must not require it — golden files are blessed
  locally and committed (same model as the echo oracle).
