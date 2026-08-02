---
"quito-release-root": minor
---

add a toolbar toggle to switch the web playground's render engine between Quito and actual OpenSCAD. The OpenSCAD path runs the official OpenSCAD 2025.03.25 WebAssembly build (Manifold backend) in a worker, loaded lazily so its ~9.6 MB wasm is only downloaded when selected. Handy for comparing our output against upstream. Limitations while on the OpenSCAD engine: no customizer, editor↔preview linking, or fast-preview (all Quito-only), and 3D models only.
