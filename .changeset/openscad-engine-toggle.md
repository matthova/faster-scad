---
"quito-release-root": minor
---

add a toolbar toggle to switch the web playground's render engine between Quito and actual OpenSCAD. The OpenSCAD path runs the official OpenSCAD 2025.03.25 WebAssembly build (Manifold backend) in a worker, loaded lazily so its ~9.6 MB wasm is only downloaded when selected. Handy for comparing our output against upstream. On the OpenSCAD engine the "Fast" toggle acts like OpenSCAD's F5 preview — a colored render showing `color(...)` — while Fast off gives a plain exact render. Limitations while on the OpenSCAD engine: no customizer or editor↔preview linking (Quito-only), and 3D models only.
