# openrscad-engine

The [OpenRSCAD](https://github.com/matthova/openrscad) geometry engine, compiled
to WebAssembly. Parse OpenSCAD-compatible `.scad` source and render solid 3D
meshes — the same Rust core that powers the
[playground](https://matthova.github.io/openrscad) — in your own browser app
or Node tool.

- **Accurate.** Output is verified bit-for-bit against stock OpenSCAD; the wasm
  build uses the pure-Rust Manifold geometry kernel.
- **Fast, incremental.** A persistent geometry cache makes warm edits
  single-digit-millisecond.
- **Self-contained.** No native dependencies, no threads, no `SharedArrayBuffer`
  — so **no COOP/COEP headers are needed**, even from a CDN.

> **Heads up — this is a 0.x release.**
> - The JS API may change between minor versions. **Pin a version** (`"openrscad-engine": "~0.1.0"`, not `^0.1.0`) if you depend on it.
> - The `.wasm` binary is a few MB. Load it in a **Web Worker** (see below) so a
>   render never blocks your UI, and let your CDN serve it gzip/brotli-compressed.

## Install

```sh
npm install openrscad-engine
```

## Quickstart (browser / bundler)

The default export is a small, safe wrapper: pass plain objects, get a plain
result back, and never manage wasm memory yourself.

```js
import { render } from "openrscad-engine";

// The first call fetches + instantiates the wasm (resolving openrscad_bg.wasm
// next to the module). Call ensureReady() yourself to preload it earlier.
const r = await render("difference() { cube(20, center=true); sphere(12); }");

if (r.ok) {
  console.log(r.triangleCount, "triangles, volume", r.volume);
  // r.positions / r.normals: non-indexed triangle soup, 9 f32 per triangle.
} else {
  console.error(r.error, r.diagnostics);
}
```

Works out of the box in Vite, webpack 5, and any ES-module bundler.

### Customizer params and include/use files

```js
await render("width = 10; cube([width, 10, 10]);", {
  params: { width: 40 },                     // numbers/booleans are stringified;
  files: { "lib.scad": "function f() = 3;" },// strings must be quoted: '"hi"'
});
```

## No-install: import straight from a CDN

Because the package publishes the browser (`web`) target, you can import it with
no build step at all. **Use the raw file path** (not `/+esm` or `esm.run`, which
mangle wasm glue) and **pin the version**:

```js
import init, { render_with_files } from
  "https://cdn.jsdelivr.net/npm/openrscad-engine@0.1.0/pkg/web/openrscad.js";

await init();                       // resolves the sibling openrscad_bg.wasm
const r = render_with_files("cube(10);", [], [], [], []);
try {
  console.log(r.triangle_count);
} finally {
  r.free();                         // raw exports are wasm-owned — free them
}
```

The CDN path exposes the **raw** wasm-bindgen exports (below); for the friendlier
wrapper, install the package instead.

## Run it in a Web Worker (recommended)

See [`examples/worker.js`](./examples/worker.js) for a ready-made module worker.
In short:

```js
// worker.js
import { render } from "openrscad-engine";
self.onmessage = async (e) => {
  const result = await render(e.data.source);
  self.postMessage({ result }, [result.positions.buffer, result.normals.buffer]);
};
```

```js
// app.js
const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
worker.onmessage = (e) => draw(e.data.result);
worker.postMessage({ source: "cube(10);" });
```

## Node

```js
import { render, version } from "openrscad-engine";   // ESM
console.log(await version());
const r = await render("sphere(8);");
```

The wasm instantiates on import in Node — no init step. CommonJS `require()`
consumers get the raw wasm-bindgen exports (call `.free()` on results). A full
example is in [`examples/node-render.mjs`](./examples/node-render.mjs).

## API

The wrapper (default export) — all methods are async and initialize the engine
on first use:

| method | returns | notes |
|---|---|---|
| `render(source, opts?)` | `Promise<RenderOutput>` | full pipeline → mesh + diagnostics |
| `exportShape2D(source, "dxf" \| "svg", opts?)` | `Promise<string>` | 2D models only |
| `parameters(source)` | `Promise<string>` | customizer schema JSON |
| `version()` | `Promise<string>` | engine version |
| `clearCache()` | `Promise<void>` | drop the persistent geometry cache |
| `ensureReady(wasmUrl?)` | `Promise<void>` | preload / point at a custom `.wasm` |

`opts` is `{ params?: Record<string, string|number|boolean>, files?: Record<string, string> }`.

`RenderOutput` includes `ok`, `error`, `positions`/`normals` (Float32Array
triangle soup, 9 f32 per triangle, flat per-face normals — parallel arrays you
own), `is2d`, `triangleCount`, `vertexCount`, `volume`, `area`, `echo`,
`warnings`, `diagnostics` (parsed `[{severity,message,start,end}]`, with byte
offsets), plus `preview` and `provenance` channels for color and editor↔preview
linking. Full types ship with the package.

### Subpath entry points (advanced)

- `openrscad-engine/web` — the raw `--target web` wasm-bindgen module (call `init()`, then the exports; `.free()` results).
- `openrscad-engine/node` — the raw `--target nodejs` module (auto-init).
- `openrscad-engine/openrscad_bg.wasm` — the wasm binary URL, for `init({ module_or_path })` with a custom loader.

Prefer the default wrapper unless you specifically need the raw surface — it
copies mesh data into values you own and frees the wasm object for you.

## License

Apache-2.0 OR MIT, at your option.
