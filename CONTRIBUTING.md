# Contributing to Quito

## Clean-room policy (mandatory)

Quito is a **clean-room** reimplementation of the OpenSCAD language. OpenSCAD is
licensed under the GPL; Quito is Apache-2.0/MIT. To keep Quito's licensing
clean, everyone — human or agent — must follow these rules:

1. **Never read, copy, or paraphrase OpenSCAD source code.** Do not open the
   OpenSCAD repository, its headers, or any GPL-derived code.
2. **Specifications come from black-box sources only:** the OpenSCAD user manual
   and Wikibook, language documentation, and observed behavior of the OpenSCAD
   command-line binary (echo output, `.csg` dumps, exported meshes).
3. **GPL test corpora are data, not code.** When we vendor upstream test data
   (openscad `tests/data`, BOSL2, etc.) it is quarantined as data-only
   submodules and used purely as oracle input/expected-output. We do not lift
   implementation logic from it.
4. If you have previously read OpenSCAD source, do not reproduce it from memory.
   Reconstruct behavior from documentation and observation instead.

When in doubt, treat OpenSCAD as a black box you can run but not read.

## Divergences

Quito targets OpenSCAD 2021.01 semantics "in spirit," and documents every
intentional divergence in [COMPAT.md](COMPAT.md) with a repro. If you change
behavior in a way that differs from upstream, add a COMPAT entry.

## Tests

- Interpreter features land with tests derived from documented behavior. Two
  oracle harnesses diff quito against a real OpenSCAD binary and run in CI against
  committed goldens: the **echo oracle** (`xtask echo`, language behavior) and the
  **geometry oracle** (`xtask geom`, mesh metrics — volume, bbox, centroid,
  component count, manifoldness, opt-in triangle count). Regenerate goldens on a
  dev machine with `xtask bless-echo` / `xtask bless-geom` (needs `openscad` on PATH).
- Geometry features land with a `corpus/geom` case blessed against OpenSCAD.
- Run `cargo test` before submitting.
