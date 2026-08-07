---
"openrscad-release-root": patch
---

`text(font=)` now selects across the bundled Liberation family — Sans/Serif/Mono in Regular/Bold/Italic/Bold Italic (e.g. `font="Liberation Serif:style=Bold"`) — matching OpenSCAD's glyphs exactly; unknown families still fall back to Liberation Sans with a warning. Bundling the full family grows the wasm engine by ~3.6 MB.
