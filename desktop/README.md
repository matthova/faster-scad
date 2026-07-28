# Quito desktop (Tauri v2)

A native desktop shell around the Quito engine. It reuses the [`web/`](../web)
playground UI, but rendering runs **natively** over Tauri IPC — the C++ Manifold
kernel instead of the browser's pure-Rust kernel — and `include`/`use` resolve
straight from disk (`OPENSCADPATH`) as well as from the open in-editor files.

## Architecture

- `src-tauri/` — the Rust/Tauri backend. Commands:
  - `render(source, dir, paramNames, paramValues, fileNames, fileContents)` →
    mesh + console + customizer schema, using a persistent geometry cache.
  - `save_model(path, format, …)` — render and write STL/OFF/OBJ/3MF/AMF/DXF/SVG.
  - `save_source(path, content)` — write `.scad` source back to disk (⌘S / Save
    As), recording a self-write marker so the watcher doesn't reload-on-save.
  - `parameters(source)` — the customizer schema JSON.
  - `open_file(path)` — read a `.scad` from disk and start watching it; returns
    its content + directory (for `include`/`use` resolution).
  - `watch_files(paths)` — watch every project file with a disk path for edits.
  - `take_pending_open()` — drain a `.scad` path passed at launch (open-with).
- **Save:** ⌘S writes the active tab back to disk (Save As when it has no path
  yet), a dot on the tab marks unsaved changes, and a native **File/Edit/View**
  menu plus a `.scad` file association (double-click to open) round out the
  desktop workflow.
- **Edit in your own editor:** an opened file is watched (via `notify`); an
  external change fires a `file-changed` event and the app reloads + re-renders.
  Self-saves are filtered out so saving from the app doesn't cause a reload.
- The frontend detects Tauri (`isTauri()` in `web/src/desktopEngine.ts`) and
  routes rendering to these commands (plus native **Open…**/**Save**/**Export**
  dialogs and menu actions); in the browser it uses the wasm worker. One UI, two
  engines.

## Build & run

Requires a Rust toolchain, `cmake` (for the C++ Manifold kernel), and the Tauri
prerequisites for your platform (on macOS, Xcode command-line tools).

```sh
# backend only (compiles the native engine + IPC surface):
cd src-tauri && cargo build

# full app (needs the Tauri CLI: `npm install` here first):
cd desktop && npm install && npm run dev      # dev, hot-reloads the web UI
cd desktop && npm run build                    # bundle installers
```

`npm run dev`/`build` build the `web/` frontend first (see `beforeBuildCommand`
in `src-tauri/tauri.conf.json`). On macOS, `npm run build` produces
`src-tauri/target/release/bundle/macos/Quito.app` and a
`…/dmg/Quito_<ver>_aarch64.dmg` installer (ad-hoc signed — for distribution
you'd add an Apple Developer signing identity + notarization). Windows/Linux
bundles come from running the same command on those platforms.
