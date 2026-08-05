---
"quito-release-root": patch
---

Web UI (M9 Phase 1b): the **orthographic projection** toggle and the **console** drawer's open state and severity filter are now remembered across reloads (they previously reset every session). Internally, persisted toggles whose value the render loop reads (Fast preview, the engine choice, editor↔preview linking) now flow through a single `usePref` hook that keeps React state, the shadow ref, and localStorage in sync atomically — closing a class of bug where a toggle could light up while renders silently ignored it.
