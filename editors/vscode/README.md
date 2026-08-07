# OpenRSCAD OpenSCAD — VS Code extension

OpenSCAD language support and a live 3D preview, powered by the
<a href="https://github.com/openrscad-cad/openrscad" target="_blank" rel="noopener noreferrer">OpenRSCAD</a> engine.

## Features

- **Diagnostics, hover, completion, and an outline** via the `openrscad-lsp` language
  server (parse/eval errors and warnings inline, signatures for built-ins and
  your own modules/functions).
- **Live 3D preview** (`OpenRSCAD: Open 3D Preview`) — geometry is rendered by the
  `openrscad-lsp` server on the native kernel and streamed to an in-editor three.js
  viewer, which re-renders as you type (debounced) or on save.
- **Export** (`OpenRSCAD: Export Model…`) to STL / 3MF / OBJ / OFF / AMF / DXF / SVG
  / PNG via the `openrscad` CLI.

## Prerequisites

Build the two Rust binaries from the repo root (see the workspace README for
toolchain setup):

```sh
cargo build --release -p openrscad-lsp   # language server
cargo build --release -p openrscad-cli   # exporter (the `openrscad` binary)
```

The extension auto-discovers both in `target/release` (then `target/debug`, then
`PATH`). Override with the `openrscad.lsp.path` / `openrscad.cli.path` settings.

## Developing / running the extension

Step by step, from the repo root:

**1. Build the Rust binaries** the extension calls (skip if `target/release`
already has them):

```sh
cargo build --release -p openrscad-lsp -p openrscad-cli
```

**2. Build the extension bundle:**

```sh
cd editors/vscode
npm install          # one time — pulls vscode-languageclient, three, esbuild
npm run compile      # typecheck + bundle dist/extension.js and media/webview.js
```

After this you should have `dist/extension.js` and `media/webview.js`. The
preview no longer needs a wasm build — the server does the rendering.

**3. Launch the Extension Development Host.** Open the `editors/vscode` folder in
VS Code and press <kbd>F5</kbd> (Run → Start Debugging). A second VS Code window
titled **[Extension Development Host]** opens with the extension loaded.

**4. Open a model.** In that new window, open any `.scad` file — e.g. from the
repo, `examples/demo.scad`. You should immediately get:

- red squiggles on syntax/eval errors, yellow on warnings (try deleting a `)`),
- hover docs over `cube`, `translate`, etc.,
- completions (<kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>Space</kbd>),
- an outline (Explorer → Outline) of your modules/functions.

**5. Open the live preview.** Run **OpenRSCAD: Open 3D Preview** from the Command
Palette (<kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd>), or click the
preview icon in the editor title bar. A 3D viewer opens beside the editor and
re-renders as you type (orbit with drag, zoom with scroll).

**6. Export.** Run **OpenRSCAD: Export Model…**, pick a format (STL/3MF/OBJ/OFF/AMF/
DXF/SVG/PNG), and it writes next to your `.scad` file.

Use `npm run watch` instead of `npm run compile` to rebuild the bundles on every
change; then reload the Extension Development Host (<kbd>Ctrl</kbd>/<kbd>⌘</kbd>+
<kbd>R</kbd> in that window) to pick up edits.

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| "OpenRSCAD language server failed to start" | Build it (`cargo build --release -p openrscad-lsp`) or set `openrscad.lsp.path` to the binary. |
| No diagnostics/hover | Confirm the file's language is **OpenSCAD** (bottom-right of the status bar); check the **OpenRSCAD Language Server** output channel. |
| Preview stuck on "Rendering…" | The server owns rendering — confirm `openrscad-lsp` started (OpenRSCAD Language Server output channel), then check the webview devtools (Command Palette → *Developer: Open Webview Developer Tools*). |
| Export says the CLI wasn't found | Build it (`cargo build --release -p openrscad-cli`) or set `openrscad.cli.path`. |

## Settings

| Setting | Default | Meaning |
| --- | --- | --- |
| `openrscad.lsp.path` | `""` | Path to `openrscad-lsp` (empty = auto-discover). |
| `openrscad.cli.path` | `""` | Path to `openrscad` CLI (empty = auto-discover). |
| `openrscad.preview.autoRefresh` | `true` | Re-render the preview as you type; off = on save only. |

## How it fits together

This extension is a thin client over the same engine the browser playground and
CLI use. The `openrscad-lsp` server is the geometry brain; the editor only supplies
display surfaces:

- **Language features** → `openrscad-lsp` (stdio LSP), which calls
  `parse → eval_program_with_params`.
- **Preview** → `openrscad-lsp` renders on the native kernel and pushes vertex
  buffers to the webview via the `openrscad/preview` notification; the webview
  (three.js) just draws them. `openrscad.startPreview` / `openrscad.stopPreview` register
  which document is live.
- **Export** → the `openrscad` CLI.
