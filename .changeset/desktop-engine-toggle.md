---
"quito-release-root": minor
---

extend the Quito⇆OpenSCAD engine toggle to the desktop app. The toolbar toggle now appears on desktop too: "Quito" uses the native C++ engine, and "OpenSCAD" renders with a **locally-installed OpenSCAD** (its fast Manifold backend) when one is available — found via the `QUITO_OPENSCAD` override, `PATH`, or the standard per-platform install locations — shelling out to export binary STL (exact) or colored OFF ($preview, F5-style). If no local OpenSCAD is installed it transparently falls back to the vendored OpenSCAD wasm build in the webview, so the toggle always works. Includes resolve from open tabs plus the file's directory (OPENSCADPATH), and exports write the OpenSCAD-produced geometry via the native save dialog. Browser behavior is unchanged (wasm engine).
