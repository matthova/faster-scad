# OpenRSCAD Playground (web)

Browser playground: CodeMirror 6 editor + live 3D preview. The engine (parser,
evaluator, geometry kernel) is the Rust core compiled to wasm and run in a
dedicated Web Worker; meshes cross to the main thread as transferable typed
arrays and render with three.js.

## Develop

```sh
# 1. Build the wasm engine (needs the Rust toolchain; see repo root).
npm run build:wasm

# 2. Install JS deps and start the dev server.
npm install
npm run dev
```

Then open the printed URL. Editing `.scad` re-renders live (200 ms debounce);
an in-flight render is cancelled (worker terminate + warm respawn) when you keep
typing. `npm run build` type-checks and produces a static bundle in `dist/` for
deployment to any static host.

## Layout

| file | role |
|---|---|
| `src/engineWorker.ts` | wasm engine host; `render(source)` → mesh + diagnostics |
| `src/engine.ts` | worker manager with cancellation (latest-wins) |
| `src/viewer.ts` | three.js orbit viewer (grid, axes, flat-shaded mesh, edges) |
| `src/scadLang.ts` | CodeMirror StreamLanguage highlighter for OpenSCAD |
| `src/stl.ts` | client-side binary STL export |
| `src/App.tsx` | layout, editor, debounce, status bar |
| `engine/` | wasm-pack output (git-ignored; regenerate with `build:wasm`) |

The kernel in the browser is the pure-Rust Manifold backend (the C++ Manifold
backend is native-only). A full Lezer grammar for the editor replaces the
StreamLanguage highlighter in a later milestone.
