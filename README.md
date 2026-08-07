# Quito

<a href="https://matthova.github.io/faster-scad/playground" target="_blank" rel="noopener noreferrer"><img src="https://img.shields.io/badge/playground-live-2ea44f" alt="playground: live"></a>
[![license: Apache-2.0 OR MIT](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue)](#license)

**Code your models. Quito turns `.scad` scripts into solid 3D geometry —
instantly, everywhere.**

Quito is a fast, from-scratch reimplementation of the
<a href="https://openscad.org" target="_blank" rel="noopener noreferrer">OpenSCAD</a> language: the same script-it-yourself CAD
workflow, a modern geometry kernel, and a single Rust core that runs in your
browser, on your desktop, in your editor, and on the command line. Edits
re-render in single-digit milliseconds, and output is verified bit-for-bit
against stock OpenSCAD.

**<a href="https://matthova.github.io/faster-scad/playground" target="_blank" rel="noopener noreferrer">▶ Try it live in your browser — no install</a>**

## Download

| Browser | Linux | Windows | Mac (native) | Mac (Intel) |
|---|---|---|---|---|
| <a href="https://matthova.github.io/faster-scad/playground" target="_blank" rel="noopener noreferrer">Open playground</a> | <a href="https://github.com/matthova/faster-scad/releases/latest/download/Quito-linux-x86_64.AppImage" target="_blank" rel="noopener noreferrer">AppImage</a> | <a href="https://github.com/matthova/faster-scad/releases/latest/download/Quito-windows-x64-setup.exe" target="_blank" rel="noopener noreferrer">Installer</a> | <a href="https://github.com/matthova/faster-scad/releases/latest/download/Quito-macos-aarch64.dmg" target="_blank" rel="noopener noreferrer">Apple Silicon</a> | <a href="https://github.com/matthova/faster-scad/releases/latest/download/Quito-macos-x64.dmg" target="_blank" rel="noopener noreferrer">Intel</a> |

Desktop links always fetch the latest release, and the app auto-updates in place.
Need `.deb` / `.rpm` / `.msi` or an older version? <a href="https://github.com/matthova/faster-scad/releases/latest" target="_blank" rel="noopener noreferrer">Browse all downloads</a>.

> **macOS: "Quito is damaged and can't be opened"?** The app isn't damaged —
> it's <a href="https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web" target="_blank" rel="noopener noreferrer">not yet notarized</a>, so Gatekeeper blocks the quarantined
> download. Drag Quito to your Applications folder, then run this once in
> Terminal to clear the quarantine flag:
>
> ```sh
> xattr -cr /Applications/Quito.app
> ```
>
> Then open it normally. (Right-click → Open won't clear the "damaged" state on
> recent macOS — use the command above.)

---

## Run it anywhere

One engine, many ways to reach it. Pick the surface that fits and start modeling.

| Surface | For whom | Get started |
|---|---|---|
| **Web** | Anyone — zero install | <a href="https://matthova.github.io/faster-scad/playground" target="_blank" rel="noopener noreferrer">Open the playground</a> |
| **Mobile / offline** | On the go | Same URL in any modern mobile browser; **install as a PWA** for offline use |
| **Desktop** | Native, no browser | Tauri app for macOS / Linux / Windows, with in-app auto-update |
| **VS Code** | Editor-native | Extension with a live 3D preview + export |
| **Any LSP editor** | Neovim, Helix, Zed, Emacs… | `quito-lsp` — diagnostics, hover, completion |
| **CLI** | Scripts & CI | `quito model.scad -o out.stl` |
| **Embed (npm)** | Your own app | `npm i quito-engine` — the wasm engine, browser or Node ([docs](packages/npm/README.md)) |

All of them drive the exact same Rust core, so a model behaves identically no
matter where you open it.

### Embed the engine in your own project

The geometry engine ships to npm as [`quito-engine`](https://www.npmjs.com/package/quito-engine)
— the same Rust core compiled to WebAssembly, for building your own viewer,
running headless renders, or embedding the playground engine:

```js
import { render } from "quito-engine";
const r = await render("difference() { cube(20, center=true); sphere(12); }");
// r.positions / r.normals: triangle soup ready for three.js / WebGL.
```

Works in bundlers (Vite/webpack), straight from a CDN (no install), and in Node.
See [`packages/npm/README.md`](packages/npm/README.md) for the worker pattern,
CDN usage, and the full API.

## What you can do

- **Write OpenSCAD-style scripts** — primitives, transforms, CSG booleans
  (`union`/`difference`/`intersection`), `hull`, `minkowski`, 2D profiles and
  extrudes, `for`/`if`, variables, user `module`s and `function`s (with
  recursion), the full expression language, `echo`/`assert`, and the `* ! # %`
  debug modifiers.
- **See it instantly** — a live 3D preview re-renders as you type, with errors
  and warnings shown inline as editor squiggles and in a console. `color()`,
  `#` highlight, and `%` background all render.
- **Tune parameters visually** — annotated variables become a **customizer**
  panel (sliders, dropdowns, checkboxes, vectors), and named **parameter sets**
  save/load presets (compatible with OpenSCAD's `.json` files).
- **Build multi-file projects** — a tab bar for several files; `include`/`use`
  resolve in-browser, and libraries like **BOSL2** are fetched on demand.
- **Animate** — `$t` playback plus frame export, with script-driven camera
  (`$vpr`/`$vpt`/`$vpd`/`$vpf`).
- **Export real files** — 3D solids to **STL / OFF / OBJ / 3MF / AMF**, 2D
  profiles to **DXF / SVG**, and rendered **PNG** images (headless on the CLI,
  no GPU required). `import()` reads meshes and 2D profiles back in.
- **Keep your work** — files, parameters, and the active tab autosave locally
  and restore on reload.

## Fast, and verifiably correct

- **~25× faster** than OpenSCAD's CGAL renderer across a six-model benchmark
  (geometric mean), and ~3–5× ahead of OpenSCAD's newest Manifold backend.
- **Warm edits render incrementally** — a content-addressed geometry cache
  recomputes only the subtrees that changed, so re-renders after a typical edit
  land in well under a millisecond.
- **Bit-for-bit verified** — a geometry oracle diffs rendered meshes against
  stock **OpenSCAD 2024.12** across a 76-case corpus (volume, bounding box,
  centroid, component count, watertight + 2-manifoldness), and BOSL2's function
  suite runs its `[[test]]` blocks in CI. Output meshes are always watertight
  and 2-manifold.

Compatibility is "OpenSCAD in spirit," not bug-for-bug. Known gaps and
intentional divergences are documented in [COMPAT.md](COMPAT.md) — a switcher
hits a documented limitation, never a silently wrong answer.

## Build from source

Everything is one Rust workspace plus a couple of Node front-ends.

### Prerequisites

Install once. The CLI/engine needs only steps 1–2; the web playground adds 3–4;
the desktop app adds 5. Commands shown for macOS (Homebrew), with Linux/Windows
notes.

1. **Rust** (stable, 1.85+) via <a href="https://rustup.rs" target="_blank" rel="noopener noreferrer">rustup</a>.
2. **cmake + a C/C++ compiler** — builds the native Manifold geometry kernel.
   macOS: `brew install cmake` (compiler ships with the Xcode CLT from step 5).
   Debian/Ubuntu: `apt install cmake build-essential`. Windows: CMake + the MSVC
   "Desktop development with C++" workload.
3. **Node.js 18+ and npm** — for the web and desktop UIs (`brew install node`).
4. **wasm-pack + the wasm target** — compiles the engine to wasm:
   ```sh
   rustup target add wasm32-unknown-unknown && cargo install wasm-pack
   ```
5. **Desktop only — Tauri deps:** macOS `xcode-select --install`; otherwise see
   the <a href="https://tauri.app/start/prerequisites/" target="_blank" rel="noopener noreferrer">Tauri prerequisites</a>.

### CLI / engine

```sh
cargo build --release
./target/release/quito examples/demo.scad -o out.stl
cargo test
```

### Web playground

```sh
cd web && npm install
npm run build:wasm      # compile the Rust engine to wasm
npm run dev             # http://localhost:5173
```

### Desktop app (Tauri)

```sh
cd desktop && npm install
npm run dev             # native app with hot-reload
npm run build           # installers → src-tauri/target/release/bundle/…
```

### Editors

The `quito-lsp` language server works in any LSP-capable editor, and
`editors/vscode` bundles it with a live 3D preview. Setup for Neovim, Helix,
Zed, Emacs, VS Code, and a CLI file-watch loop is in
[`docs/ide-integration.md`](docs/ide-integration.md).

## Repo layout

| crate | responsibility |
|---|---|
| `quito-syntax` | lexer + parser → typed AST; customizer schema |
| `quito-ir` | CSG tree/DAG node types |
| `quito-eval` | tree-walk interpreter + bytecode VM: AST → CSG tree; `text()` |
| `quito-geom` | fragments, tessellation, `Kernel` trait, Manifold backend, mesh/2D I/O |
| `quito-cli` | the `quito` binary |
| `quito-wasm` | wasm-bindgen engine surface (`render(source)` → mesh + diagnostics) |
| `quito-lsp` | LSP language server: diagnostics, hover, completion, render command |

`web/` (live at <a href="https://matthova.github.io/faster-scad/" target="_blank" rel="noopener noreferrer">https://matthova.github.io/faster-scad/</a>), `desktop/` (Tauri),
and `editors/vscode/` are thin front-ends over the same core. `packages/npm/`
publishes the wasm engine as [`quito-engine`](packages/npm/README.md) for use in
other projects.

## Contributing

Contributions welcome — see [CONTRIBUTING.md](CONTRIBUTING.md). Quito is a
clean-room reimplementation: **no OpenSCAD (GPL) source is ever consulted.**

## License

Dual-licensed under either [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
