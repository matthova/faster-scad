---
"quito-release-root": patch
---

Web UI (M9 Phase 0): fix four defects. Dropped **binary STL** files are now detected and refused with a clear message instead of being UTF-8-mangled and fed to the parser as garbage (ASCII STL still imports). In the browser, **⌘S** now downloads the active `.scad` instead of being a swallowed no-op, and works even when the editor isn't focused. The **Display ▾** indicator now lights when the section plane or dimensions are on (it previously ignored both, so enabling the geometry-hiding section plane left the menu dark). Switching to the **OpenSCAD engine** on a slow connection no longer aborts the first render with a misleading "model too complex" message — the ~10 MB download runs outside the render watchdog with a visible downloading banner. Also corrected the `#` highlight-modifier label in the Model panel (was mislabelled `!`).
