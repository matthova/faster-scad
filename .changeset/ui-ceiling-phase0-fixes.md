---
"quito-release-root": patch
---

Web UI (M9 Phase 0): fix four defects. Dropped **binary STL** files are now detected and refused with a clear message instead of being UTF-8-mangled and fed to the parser as garbage (ASCII STL still imports). In the browser, **⌘S** now downloads the active `.scad` instead of being a swallowed no-op, and works even when the editor isn't focused. The **Display ▾** indicator now lights when the section plane or dimensions are on (it previously ignored both, so enabling the geometry-hiding section plane left the menu dark). Switching to the **OpenSCAD engine** on a slow connection no longer aborts the first render with a misleading "model too complex" message — the ~10 MB download runs outside the render watchdog with a visible downloading banner. Also corrected the `#` highlight-modifier label in the Model panel (was mislabelled `!`).

Accessibility: keyboard focus is now always visible (a `:focus-visible` ring that clears 3:1 in both themes — there were previously zero focus styles), light-theme text colours (`--muted`, `--warn`, the amber accent in text/border roles) were darkened to meet WCAG contrast, control boundaries and the floating **⤢ Fit** button use a stronger border token so they don't vanish, the tab close (✕) is no longer a faint 2.22:1 glyph, and animations honour `prefers-reduced-motion` (including the viewport fly-to camera).
