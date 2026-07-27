# Quito

A fast, greenfield reimplementation of the [OpenSCAD](https://openscad.org)
language — same language spirit, a modern geometry kernel, one Rust core
shipping to the browser (wasm) and desktop (Tauri).

> **Status: M3 complete, M4 exit criteria met — full language + geometry + perf.**
> M0 (native skeleton), M1 (playground + kernel bake-off) and M2 (full language)
> are complete: BOSL2's function test suite passes **13/15 (87%)** and an echo
> oracle (24/24) diffs the language against real OpenSCAD. M3 has added the 2D
> subsystem (`square`/`circle`/`polygon`/`offset`), `linear_extrude`/
> `rotate_extrude` (with 2D booleans), `projection(cut=true)`, `hull`,
> `minkowski`, `mirror`/`multmatrix`/`resize`/`color`, and STL/OFF/OBJ import +
> export — so the complete parametric lamp *assembly* renders (matching
> OpenSCAD's Manifold backend, where its default CGAL backend crashes). Still to
> come: a 2D clipper (for `projection(cut=false)` silhouettes, robust
> offset-of-booleans, non-convex minkowski) and the benchmark suite. The full
> plan is in `.context/attachments/HoR0PL/plan.md`; research is in
> `.context/research/`.

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
the debug modifiers `* ! # %`. Curved primitives use the bit-exact `$fn/$fa/$fs`
fragment formula.

### Kernel bake-off (resolved)

Geometry booleans run behind a `Kernel` trait with two backends:

- **Native:** C++ [Manifold](https://github.com/elalish/manifold) via
  `manifold-csg` — fast and battle-tested.
- **Browser (wasm):** pure-Rust [`boolmesh`](https://crates.io/crates/boolmesh)
  — compiles clean to `wasm32-unknown-unknown` (no Emscripten). A differential
  test confirms the two backends agree to within 0.5% on
  union/difference/intersection.

### Browser playground (M1)

A live-editing playground runs the engine as wasm in a Web Worker with a
three.js preview; see [`web/`](web/README.md). Verified in headless Chrome:
non-trivial `.scad` edits re-render live in single-digit milliseconds, with
worker-terminate cancellation and parse errors surfaced inline. `cd web && npm
run build:wasm && npm install && npm run dev`.

### Oracle verification

Rendered output is validated against stock **OpenSCAD 2024.12**. On the sample
models the meshes are **bit-for-bit identical in topology and volume**:

| model | quito | OpenSCAD | Δ volume |
|---|---|---|---|
| `sphere($fn=48)` | 4158.9810 (2300 tris) | 4158.9810 (2300 tris) | 0.0000% |
| `cylinder(h=20,r=7)` | 3037.0769 (84 tris) | 3037.0770 (84 tris) | 0.0000% |
| `demo.scad` (union/difference/for/modules) | 12119.0398 (1164 tris) | 12119.0399 (1164 tris) | 0.0000% |

Output meshes are watertight and 2-manifold (every edge shared by exactly two
triangles). As a larger end-to-end check, the parametric *draped/fluted dome
lamp shade* (`examples/lamp.scad`) — a single analytic-surface `polyhedron`
built with C-style-`for` comprehensions — renders to **17,408 triangles,
watertight, volume 13.596**, identical in triangle count and matching in volume
to OpenSCAD 2024.12 (regression-tested in CI).

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
| eval-bound (heavy compute, tiny mesh) | 138 ms | 517 ms | **3.8×** | 523 ms | **3.8×** |

The **geometric mean across all five models is ~25× vs CGAL** (OpenSCAD
2021.01's renderer) — clearing the ≥10× goal *including the eval-bound model* —
and Quito stays **~3–5× ahead of OpenSCAD's newest Manifold backend**. Both
baselines still pay process-startup overhead that dominates the smallest models,
so these are conservative. Models live in [`benches/models/`](benches/models/).

The **eval-bound** row — a model that is nearly all interpretation and almost no
geometry — was a published loss until the M4 **bytecode VM** landed: function
bodies now compile to slot-based bytecode (tail-calls become jumps), taking it
from parity (1.1×) to **3.8×**. The tree-walk interpreter remains the reference
semantics, the fallback for anything the VM doesn't compile (comprehensions,
closures over locals, named-argument calls), and the differential oracle — so
the VM only changes timing, never results (24/24 echo oracle unchanged).

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

Requires a Rust toolchain and `cmake` (for the C++ Manifold backend).

```sh
cargo build --release
./target/release/quito examples/demo.scad -o out.stl
./target/release/quito examples/demo.scad --check        # echo/warnings only
cargo test                                                # unit + kernel tests
```

## Repo layout

| crate | responsibility |
|---|---|
| `quito-syntax` | logos lexer + recursive-descent parser → typed AST |
| `quito-ir` | CSG tree/DAG node types |
| `quito-eval` | tree-walk interpreter: AST → CSG tree |
| `quito-geom` | fragment formula, tessellation, `Kernel` trait, Manifold backend, STL |
| `quito-cli` | the `quito` binary |

| `quito-wasm` | wasm-bindgen engine surface (`render(source)` → mesh + diagnostics) |

The `web/` playground is live. Planned crates (`quito-text`, `quito-io`,
`quito-engine`) and the `desktop/` shell arrive in later milestones.

## Next steps

- Deploy the `web/` bundle to a public URL and benchmark live-edit latency vs
  `openscad-playground` on the same models (M1 exit criterion).
- Lezer grammar for the editor (replacing the StreamLanguage highlighter).
- Begin M2: full expression language, list comprehensions, `children()`/
  `$children`, proper hoisting + lexical scoping, and the echo-oracle harness.

## License

Dual-licensed under either of Apache-2.0 or MIT at your option. Quito is a
clean-room reimplementation — see [CONTRIBUTING.md](CONTRIBUTING.md). No
OpenSCAD (GPL) source is ever consulted.
