# Editor / IDE integration

The web playground isn't the only way to drive the engine. Quito ships three
front-ends for working in a code editor instead, from zero-setup to full IDE:

1. **[CLI + a file watcher](#1-cli--a-file-watcher)** — works today, no plugins.
2. **[`quito-lsp` language server](#2-quito-lsp-language-server)** — diagnostics,
   hover, completion, and an outline in any LSP-capable editor.
3. **[VS Code extension](#3-vs-code-extension)** — the language server plus an
   in-editor live 3D preview and export.

All three are thin front-ends over the same `parse → eval_program_with_params →
render` pipeline the playground and CLI use.

---

## 1. CLI + a file watcher

The `quito` binary reads a `.scad` file and writes a mesh/vector/image by output
extension (`.stl` `.off` `.obj` `.3mf` `.amf` `.dxf` `.svg` `.png`). Pair it with
your editor's watch/build hook and a preview viewer for a zero-plugin loop.

Build it once:

```sh
cargo build --release -p quito-cli    # → target/release/quito
```

### Watch recipes

**<a href="https://github.com/watchexec/watchexec" target="_blank" rel="noopener noreferrer">`watchexec`</a>** (any editor):

```sh
watchexec -e scad -- ./target/release/quito model.scad -o model.stl
# or a rendered image:
watchexec -e scad -- ./target/release/quito model.scad -o preview.png --imgsize 900,700
```

**<a href="https://eradman.com/entrproject/" target="_blank" rel="noopener noreferrer">`entr`</a>:**

```sh
ls *.scad | entr ./target/release/quito /_ -o model.stl
```

**VS Code task** (`.vscode/tasks.json`) — build on every save via the
<a href="https://marketplace.visualstudio.com/items?itemName=Gruntfuggly.triggertaskonsave" target="_blank" rel="noopener noreferrer">Trigger Task on Save</a>
extension, or run once with **Run Task → Quito: render**:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Quito: render",
      "type": "shell",
      "command": "${workspaceFolder}/target/release/quito",
      "args": ["${file}", "-o", "${fileDirname}/${fileBasenameNoExtension}.stl"],
      "problemMatcher": []
    }
  ]
}
```

**Neovim** (autocmd on save):

```lua
vim.api.nvim_create_autocmd("BufWritePost", {
  pattern = "*.scad",
  callback = function(a)
    local out = a.file:gsub("%.scad$", ".stl")
    vim.fn.jobstart({ "quito", a.file, "-o", out })
  end,
})
```

### Previewing the output

- **PNG** (`-o preview.png`): open in any auto-refreshing image viewer. Frame with
  `--imgsize W,H`, `--camera …`, `--projection ortho`, `--viewall`.
- **STL/3MF**: any hot-reloading mesh viewer, e.g.
  <a href="https://f3d.app/" target="_blank" rel="noopener noreferrer">`f3d`</a> (`f3d model.stl`) or the VS Code
  <a href="https://marketplace.visualstudio.com/items?itemName=slevesque.vscode-3dviewer" target="_blank" rel="noopener noreferrer">3D Viewer</a>
  extension.

`include`/`use` resolve relative to the input file and each `OPENSCADPATH`
directory, exactly like OpenSCAD.

> The CLI starts a fresh process per render (cold geometry cache). For a warm,
> incremental edit loop, use the language server's render command or the VS Code
> extension below.

---

## 2. `quito-lsp` language server

`quito-lsp` speaks the <a href="https://microsoft.github.io/language-server-protocol/" target="_blank" rel="noopener noreferrer">Language Server Protocol</a>
over stdio, so one server works across VS Code, Neovim, Helix, Zed, Emacs, and
any other LSP client. It provides:

| Feature | What you get |
| --- | --- |
| **Diagnostics** | Parse/eval errors and warnings inline, on open/change/save. |
| **Hover** | Signature + docs for built-ins and your own modules/functions/vars. |
| **Completion** | Built-ins plus in-document symbols. |
| **Document symbols** | An outline of the file's `module`/`function`/variable defs. |
| **`quito.render` command** | `workspace/executeCommand` that renders the document to a file (STL/OFF/OBJ/3MF/AMF/DXF/SVG) and returns stats — the hook an editor uses to drive a preview. |

`include`/`use` resolve against open editor buffers first (so unsaved edits are
honored), then disk + `OPENSCADPATH`. Evaluation runs on a 256 MiB-stack worker
thread, so deeply recursive libraries (BOSL2, etc.) don't overflow.

Build it:

```sh
cargo build --release -p quito-lsp    # → target/release/quito-lsp
```

### Wiring it up

**Neovim** (0.11+ `vim.lsp.config`):

```lua
vim.filetype.add({ extension = { scad = "openscad" } })
vim.lsp.config.quito = {
  cmd = { "quito-lsp" },       -- or the absolute path to target/release/quito-lsp
  filetypes = { "openscad" },
  root_markers = { ".git" },
}
vim.lsp.enable("quito")
```

**Helix** (`languages.toml`):

```toml
[language-server.quito-lsp]
command = "quito-lsp"

[[language]]
name = "openscad"
scope = "source.openscad"
file-types = ["scad"]
language-servers = ["quito-lsp"]
```

**Zed** / **Emacs (`eglot`/`lsp-mode`)** / **Sublime (LSP)**: register a language
server whose command is `quito-lsp` for the `.scad` file type. The server needs no
initialization options.

**Rendering from an editor:** send a `workspace/executeCommand` for `quito.render`
with `arguments: [documentUri, outputPath?]`. It returns
`{ ok, path, triangles, vertices, volume, area }`. Omit `outputPath` to write the
source's name with a `.stl` extension.

---

## 3. VS Code extension

The `editors/vscode` extension bundles the language server client, a **live 3D
preview** (the wasm engine + a three.js viewer in a webview, re-rendering as you
type), and an **export** command. See
[`editors/vscode/README.md`](../editors/vscode/README.md) for build and run
instructions.

Quick start:

```sh
cargo build --release -p quito-lsp -p quito-cli   # binaries the extension calls
cd editors/vscode
npm install
npm run build:wasm     # preview engine → media/engine  (needs wasm-pack)
npm run compile        # typecheck + bundle
# press F5 in VS Code to launch an Extension Development Host
```
