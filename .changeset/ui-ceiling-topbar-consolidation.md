---
"quito-release-root": minor
---

Web UI (M9 Phase 4): the toolbar is reorganized into grouped menus so it fits on **one row** (it previously wrapped to two even at 1280px). Controls now live under **Project ▾** (New, Import…, Share, Download .scad; Open/Save/Save As on desktop), **Quality ▾** (Draft/Normal/Fine/Custom with `$fn`/`$fa`/`$fs`, current preset shown in the label), **Export ▾** (a split button: one-click export in the current format, with the format list, PNG, and animation Frames in the caret menu), and **? ▾** (help, theme, GitHub, version). The animation transport (play, `$t` scrubber, FPS, Steps) moves to a strip **below the viewport** that stays collapsed until your script uses `$t`. New: browser **Import…** via a file picker (was drag-only). Menus close on action and open on the group triggers; the row no longer wraps at 1024px.
