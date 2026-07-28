# Quito

[![playground: live](https://img.shields.io/badge/playground-live-2ea44f)](https://matthova.github.io/faster-scad/)

A fast, greenfield reimplementation of the [OpenSCAD](https://openscad.org)
language — same language spirit, a modern geometry kernel, one Rust core
shipping to the browser (wasm) and desktop (Tauri). **Try it live, no install,
in your browser: <https://matthova.github.io/faster-scad/>.**

> **Status: M4 met, M5 (playground → product) landing.**
> M0–M2 (native skeleton, playground+kernel bake-off, full language) and M3 (full
> geometry) are complete; M4 met both perf exits (warm-edit <100 ms via a
> geometry cache; ~25× geomean vs OpenSCAD 2021.01/CGAL including an eval-bound
> model, via a bytecode VM). M5 has turned the playground into a product: a
> **customizer** (annotation-driven parameter UI), **multi-file projects** with
> in-browser `include`/`use` and lazy library fetching, **STL/OFF/OBJ export**, a
> **console**, **localStorage persistence**, and an installable/offline **PWA**.
> An echo oracle (24/24) diffs the language against real OpenSCAD, a geometry
> oracle (69/69) diffs rendered meshes against OpenSCAD 2024.12, and BOSL2's
> function suite runs every `[[test]]` block (422/513 pass, baseline-gated in CI
> so a regression can't hide behind a green file). The full plan is in
> `.context/attachments/HoR0PL/plan.md`; research is in `.context/research/`.

## What works today (M0 native)

A `.scad` file flows end-to-end through the native pipeline:

```
source → lex/parse (quito-syntax) → tree-walk eval (quito-eval)
       → CSG tree (quito-ir) → tessellate + kernel booleans (quito-geom)
       → mesh → STL (quito-cli)
```

Language subset: `cube` / `sphere` / `cylinder` (incl. cones, `d`/`r1`/`r2`),
`translate` / `rotate` / `scale`, `union` / `difference` / `intersection`,
`for` / `if` / `else`, variables with last-assignment-wins hoisting, user
`module`s and `function`s (incl. recursion), `let`, ranges, vectors, the full
expression language (arithmetic, comparison with undef propagation, ternary,
indexing, `.x/.y/.z`), a core set of math/list builtins, `echo`, `assert`, and
the debug modifiers `* ! # %`. `color()` (named/hex/vector + alpha) and the
`#`/`%` modifiers are rendered in the preview — colored subtrees, `#` highlight
(translucent red), `%` background (translucent gray, excluded from export) — and
`color()` carries into per-object 3MF export. Curved primitives use the bit-exact
`$fn/$fa/$fs` fragment formula.

### Kernel bake-off (resolved)

Geometry booleans run behind a `Kernel` trait with two backends:

- **Native:** C++ [Manifold](https://github.com/elalish/manifold) via
  `manifold-csg` — fast and battle-tested.
- **Browser (wasm):** pure-Rust [`manifold-rust`](https://crates.io/crates/manifold-rust)
  — compiles clean to `wasm32-unknown-unknown` (no Emscripten). A differential
  test confirms the two backends agree to within 0.5% on
  union/difference/intersection.

### Browser playground

The engine runs as wasm in a Web Worker with a three.js preview; edits re-render
live in single-digit milliseconds, with worker-terminate cancellation and errors
reported in the status bar and console **and as inline editor squiggles** (red
for errors, yellow for warnings, cleared on success). Run it locally with
`cd web && npm install && npm run build:wasm && npm run dev`; see
[`web/`](web/README.md).

**Product features (M5):**

- **Customizer** — annotated top-level variables become an editable control
  panel (grouped), and changes re-render live. Supported annotations:

  | annotation | control |
  |---|---|
  | `x = 5;` | number spinbox |
  | `x = 5;  // [0:10]` | slider |
  | `x = 5;  // [0:0.5:10]` | slider with step |
  | `x = 2;  // [0, 1, 2, 3]` | dropdown (values) |
  | `m = 1;  // [0:Off, 1:On]` | dropdown (labelled) |
  | `on = true;` | checkbox |
  | `s = "hi"; // 8` | text (max length) |
  | `v = [1,2,3];` | vector |

  `/* [Group] */` starts a group (`[Hidden]` drops params); a `//` comment on the
  line above a variable becomes its label. The same overrides are available on
  the CLI: `quito model.scad -D width=20 -o out.stl`.
- **Parameter sets** — save/apply named presets in the UI and import/export
  OpenSCAD's `.json` parameter-set files; on the CLI, `quito model.scad -p
  sets.json -P Big -o out.stl`.
- **Animation** — `$t` playback in the UI, plus frame export: an **Export frames**
  button (zip of PNGs) and the CLI `quito model.scad -o out.png --animate 60`.
  Scripts can read and drive the camera via `$vpr`/`$vpt`/`$vpd`/`$vpf`.
- **Multi-file projects** — a tab bar for several files; the first is rendered
  and the rest are libraries. `include`/`use` resolve between files in-browser,
  and unknown paths are fetched lazily (bundled `/lib/`, or a CDN by prefix —
  e.g. `BOSL2/…` from jsDelivr) with transitive resolution + caching.
- **Export** — the format dropdown adapts to the model: 3D solids export to STL
  (binary), OFF, OBJ, 3MF, or AMF; 2D profiles export to DXF or SVG. Available
  in the playground, the desktop app, and the CLI. `import()` reads meshes
  (STL/OFF/OBJ/3MF/AMF, including OpenSCAD's ZIP64+deflate 3MF) and 2D profiles
  (DXF/SVG).
- **PNG image export** — a **Save PNG** button captures the viewer in the
  playground and desktop app, and the CLI renders headlessly (no GPU) with a
  pure-Rust rasterizer: `quito model.scad -o out.png --imgsize 800,600
  --camera=…` (OpenSCAD-style `--camera`/`--projection`/`--viewall`/
  `--autocenter`), colored to match the preview.
- **Console** — echo / warnings / errors, color-coded.
- **Examples** — an Examples menu loads curated sample projects (CSG, twisted
  extrusion, text, a 2D gasket for DXF/SVG, a `$t` animation, and a BOSL2 model).
- **Persistence** — files, parameter values, and the active tab autosave to
  `localStorage` and restore on reload.
- **PWA** — installable, and the app shell + engine are cached for offline use.

### Oracle verification

Rendered output is validated against stock **OpenSCAD 2024.12**. On the sample
models the meshes are **bit-for-bit identical in topology and volume**:

| model | quito | OpenSCAD | Δ volume |
|---|---|---|---|
| `sphere($fn=48)` | 4158.9810 (2300 tris) | 4158.9810 (2300 tris) | 0.0000% |
| `cylinder(h=20,r=7)` | 3037.0769 (84 tris) | 3037.0770 (84 tris) | 0.0000% |
| `demo.scad` (union/difference/for/modules) | 12119.0398 (1164 tris) | 12119.0399 (1164 tris) | 0.0000% |

Output meshes are watertight and 2-manifold (every edge shared by exactly two
triangles). This is enforced continuously by the **geometry oracle**
(`cargo run -p xtask -- geom`): a 69-case corpus (`corpus/geom`) spanning
primitives, transforms, booleans, extrudes, the 2D pipeline, hull/minkowski,
imports, and `surface`, each blessed from OpenSCAD 2024.12 and checked in CI on
volume (±0.1%), bbox and signed centroid (±0.01 mm), connected-component count,
watertight+2-manifoldness, and opt-in triangle count. Goldens are committed, so
CI needs no OpenSCAD; regenerate them with `xtask bless-geom` on a dev machine.
As a larger end-to-end check, the parametric *draped/fluted dome lamp shade*
(`examples/lamp.scad`) — a single analytic-surface `polyhedron` built with
C-style-`for` comprehensions — renders to **17,408 triangles, watertight, volume
13.596**, identical in triangle count and matching in volume to OpenSCAD 2024.12.

### Benchmarks (dual baseline)

Full-process wall-clock, best of 3 runs, on an Apple Silicon laptop, comparing
the release `quito` binary against OpenSCAD 2024.12's two render backends on the
same `.scad` files (`cargo build --release && cargo run -p xtask -- bench`):

| model | quito | OpenSCAD (CGAL) | speed-up | OpenSCAD (Manifold) | speed-up |
|---|--:|--:|--:|--:|--:|
| lamp shade (analytic polyhedron) | 34 ms | 172 ms | **5.1×** | 180 ms | **5.3×** |
| booleans (grid of holes in a slab) | 53 ms | 20,000 ms | **377×** | 172 ms | **3.2×** |
| rounded (minkowski + hull) | 24 ms | 350 ms | **14×** | 52 ms | **2.1×** |
| gears (extrude/revolve heavy) | 17 ms | 1,925 ms | **113×** | 60 ms | **3.5×** |
| eval-bound (heavy compute, tiny mesh) | 97 ms | 517 ms | **5.3×** | 516 ms | **5.3×** |

The **geometric mean across all five models is ~25× vs CGAL** (OpenSCAD
2021.01's renderer) — clearing the ≥10× goal *including the eval-bound model* —
and Quito stays **~3–5× ahead of OpenSCAD's newest Manifold backend**. Both
baselines still pay process-startup overhead that dominates the smallest models,
so these are conservative. Models live in [`benches/models/`](benches/models/).

The **eval-bound** row — a model that is nearly all interpretation and almost no
geometry — was a published loss until the M4 **bytecode VM** landed: function
bodies compile to slot-based bytecode (tail-calls become jumps), with an inlined
number-op-number fast path and a hoisted self-call check, taking it from parity
(1.1×) to **5.3×**. The tree-walk interpreter remains the reference semantics,
the fallback for anything the VM doesn't compile (comprehensions, closures over
locals, named-argument calls), and the differential oracle — so the VM only
changes timing, never results (24/24 echo oracle unchanged).

#### Warm-edit latency (M4 cache)

A content-addressed geometry cache (`GeomCache`, keyed by a structural hash of
each CSG subtree) makes re-renders incremental: only subtrees whose structure
changed are recomputed, the rest are `Mesh` clones, and identical subtrees in one
render are deduplicated (CSE). Re-rendering after an edit that doesn't change
geometry (in-process, native kernel):

| model | cold | warm (cache reused) | speed-up |
|---|--:|--:|--:|
| booleans | 47 ms | 0.08 ms | 597× |
| rounded (minkowski) | 159 ms | 0.01 ms | 14,000× |
| gears | 6.8 ms | 0.04 ms | 153× |

Comfortably inside the **<100 ms warm-edit** M4 target. The cache is persisted
across edits in the playground's Web Worker; a real geometry edit re-renders only
the changed root-to-leaf path.

## Build & run

### Prerequisites

Install these once. Commands are shown for macOS (Homebrew), with equivalents
noted for Linux/Windows. The CLI/engine needs only steps 1–2; the browser
playground adds 3–4; the desktop app adds 5.

1. **Rust** (stable, 1.85+) via [rustup](https://rustup.rs):
   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. **cmake + a C/C++ compiler** — builds the C++ Manifold kernel used by the
   native engine:
   ```sh
   brew install cmake          # macOS — the compiler comes with the Xcode CLT (step 5)
   # Debian/Ubuntu: sudo apt install cmake build-essential
   # Windows:       install CMake + the MSVC "Desktop development with C++" workload
   ```
3. **Node.js 18+ and npm** — for the browser playground and the desktop UI:
   ```sh
   brew install node           # macOS (or use nvm / your OS package manager)
   ```
4. **wasm-pack + the wasm target** — compiles the engine to wasm for the UI:
   ```sh
   rustup target add wasm32-unknown-unknown
   cargo install wasm-pack
   ```
5. **Desktop only — Tauri platform deps:**
   - **macOS:** `xcode-select --install` (Xcode command-line tools).
   - **Linux/Windows:** see the
     [Tauri prerequisites](https://tauri.app/start/prerequisites/) —
     `webkit2gtk` + `build-essential` on Linux; the WebView2 runtime + MSVC
     build tools on Windows.

### CLI / engine (prereqs 1–2)

```sh
cargo build --release
./target/release/quito examples/demo.scad -o out.stl
./target/release/quito examples/demo.scad --check        # echo/warnings only
cargo test                                               # unit + kernel tests
```

### Browser playground (prereqs 1–4)

```sh
cd web
npm install
npm run build:wasm    # compile the Rust engine to wasm
npm run dev           # serves http://localhost:5173
```

### Desktop app — Tauri (prereqs 1–5)

The desktop build compiles the web UI (and wasm engine) for you.

```sh
cd desktop
npm install           # first time only — installs the Tauri CLI
npm run dev           # launch the native app with hot-reload
npm run build         # bundle installers → src-tauri/target/release/bundle/…
```

## Repo layout

| crate | responsibility |
|---|---|
| `quito-syntax` | logos lexer + recursive-descent parser → typed AST; customizer schema |
| `quito-ir` | CSG tree/DAG node types |
| `quito-eval` | tree-walk interpreter + bytecode VM: AST → CSG tree; `text()` |
| `quito-geom` | fragment formula, tessellation, `Kernel` trait, Manifold backend, mesh/2D I/O |
| `quito-cli` | the `quito` binary |
| `quito-wasm` | wasm-bindgen engine surface (`render(source)` → mesh + diagnostics) |

The `web/` playground is live at <https://matthova.github.io/faster-scad/>, and
the `desktop/` Tauri shell ships alongside it — both drive the same Rust core.

## Roadmap

M0–M5 are complete (native skeleton, playground + kernel bake-off, full language,
full geometry, perf, and the M5 product surface). The candidate post-M5 tracks —
trustworthy geometry (a geometry oracle), the switcher experience (desktop Save,
inline diagnostics, `color`/`#`/`%` rendering), and CI hardening — are written up
in [`docs/roadmap/`](docs/roadmap/). Known compatibility gaps and intentional
divergences are tracked in [COMPAT.md](COMPAT.md).

## License

Dual-licensed under either of Apache-2.0 or MIT at your option. Quito is a
clean-room reimplementation — see [CONTRIBUTING.md](CONTRIBUTING.md). No
OpenSCAD (GPL) source is ever consulted.
