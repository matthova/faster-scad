# Quito desktop (Tauri v2)

A native desktop shell around the Quito engine. It reuses the [`web/`](../web)
playground UI, but rendering runs **natively** over Tauri IPC — the C++ Manifold
kernel instead of the browser's pure-Rust kernel — and `include`/`use` resolve
straight from disk (`OPENSCADPATH`) as well as from the open in-editor files.

## Architecture

- `src-tauri/` — the Rust/Tauri backend. Commands:
  - `render(source, dir, paramNames, paramValues, fileNames, fileContents)` →
    mesh + console + customizer schema, using a persistent geometry cache.
  - `save_model(path, format, …)` — render and write STL/OFF/OBJ.
  - `parameters(source)` — the customizer schema JSON.
- The frontend detects Tauri (`isTauri()` in `web/src/desktopEngine.ts`) and
  routes rendering to these commands; in the browser it uses the wasm worker.
  One UI, two engines.

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
in `src-tauri/tauri.conf.json`).
