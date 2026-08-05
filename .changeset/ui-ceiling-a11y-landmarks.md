---
"quito-release-root": patch
---

Web UI (M9): accessibility — the app now exposes a `<main>` landmark and a page heading, the code editor has an accessible name, the engine and Fast toggles report their pressed state, and the status bar announces render outcomes to screen readers. The automated axe-core CI gate is tightened to enforce these (landmark, heading, and control-name rules) on every PR.
