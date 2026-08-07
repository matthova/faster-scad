# Vendored OpenSCAD WebAssembly build

These files are an **official prebuilt OpenSCAD WebAssembly build**, copied
verbatim from the upstream nightly artifacts:

- Source: https://files.openscad.org/playground/OpenSCAD-2025.03.25.wasm24456-WebAssembly-web.zip
- Build: OpenSCAD **2025.03.25** (wasm24456), with the **Manifold** geometry
  backend — the same build the official OpenSCAD web playground ships.
- Files: `openscad.js` (Emscripten loader, ES module) and `openscad.wasm`.

They are shipped so the playground can offer OpenSCAD itself as an alternate
render engine alongside OpenRSCAD (toggled in the toolbar). They are loaded lazily —
only when a user selects the OpenSCAD engine — so the ~9.6 MB `openscad.wasm` is
never downloaded by default, and it is excluded from the PWA precache.

## License

OpenSCAD is **free software licensed under the GNU General Public License,
version 2 or later (GPL-2.0-or-later)**. These are the *compiled* binaries only.
The corresponding source is available at:

- https://github.com/openscad/openscad
- https://github.com/openscad/openscad-wasm

This is clean-room-safe for OpenRSCAD: we treat OpenSCAD strictly as an opaque
external binary (same as the native `openscad` oracle the Rust tests shell out
to). No OpenSCAD source is read or paraphrased into OpenRSCAD's own code.

## Updating

To refresh, download a newer `*-WebAssembly-web.zip` from
https://files.openscad.org/playground/ (the URL is also tracked in the OpenSCAD
playground's `libs-config.json`) and replace `openscad.js` + `openscad.wasm`
here, then bump `OPENSCAD_VERSION` in `web/src/openscadWorker.ts`.
