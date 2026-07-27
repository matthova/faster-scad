# Quito

A fast, greenfield reimplementation of the [OpenSCAD](https://openscad.org)
language — same language spirit, a modern geometry kernel, one Rust core
shipping to the browser (wasm) and desktop (Tauri).

> **Status: M0 — native walking skeleton complete and oracle-verified.**
> The full plan lives in `.context/attachments/HoR0PL/plan.md`; research
> dossiers are in `.context/research/`.

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

Geometry booleans run on the C++ [Manifold](https://github.com/elalish/manifold)
kernel via the `manifold-csg` bindings, behind a `Kernel` trait (the seam for
the planned pure-Rust-vs-C++ kernel bake-off).

### Oracle verification

Rendered output is validated against stock **OpenSCAD 2024.12**. On the sample
models the meshes are **bit-for-bit identical in topology and volume**:

| model | quito | OpenSCAD | Δ volume |
|---|---|---|---|
| `sphere($fn=48)` | 4158.9810 (2300 tris) | 4158.9810 (2300 tris) | 0.0000% |
| `cylinder(h=20,r=7)` | 3037.0769 (84 tris) | 3037.0770 (84 tris) | 0.0000% |
| `demo.scad` (union/difference/for/modules) | 12119.0398 (1164 tris) | 12119.0399 (1164 tris) | 0.0000% |

Output meshes are watertight and 2-manifold (every edge shared by exactly two
triangles).

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

Planned crates (`quito-text`, `quito-io`, `quito-engine`, `quito-wasm`) and the
`web/` + `desktop/` shells arrive in later milestones.

## Next step

Per the roadmap, M1 is the browser playground. Its blocker is the **kernel
bake-off**: `manifold-csg` is C++ and does not target
`wasm32-unknown-unknown`, so the wasm build needs a pure-Rust kernel
(`manifold-rust`) behind the same `Kernel` trait. That evaluation is the first
M1 task.

## License

Dual-licensed under either of Apache-2.0 or MIT at your option. Quito is a
clean-room reimplementation — see [CONTRIBUTING.md](CONTRIBUTING.md). No
OpenSCAD (GPL) source is ever consulted.
