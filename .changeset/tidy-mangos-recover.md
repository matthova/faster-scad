---
"quito-release-root": patch
---

fix crash-recovery so a too-heavy model can't freeze the app on every launch: the render watchdog no longer clears its own recovery sentinel, and safe mode now survives repeated relaunches until a render actually finishes (previously a render heavier than the 20s watchdog re-triggered the freeze on startup, since the watchdog's own timeout wiped the "skip auto-render" flag)
