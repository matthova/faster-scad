---
"quito-release-root": minor
---

recover from heavy-geometry render freezes: a render-in-progress indicator with a Stop button, a watchdog that auto-stops runaway renders, and a startup recovery banner that skips auto-rendering a project whose last render never finished (so a too-heavy script no longer freezes the app on every reload)
