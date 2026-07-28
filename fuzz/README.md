# quito-fuzz

libFuzzer targets for the parts of the engine that ingest **untrusted public
input**: the parser and the mesh/vector importers. The playground runs these
crates in a browser worker (a panic crashes the worker) and they also run
natively in the CLI/desktop, so the contract is: *never panic, never hang, never
OOM — only return an error or an empty/partial result.*

This is a standalone crate (its own `[workspace]`) so the nightly-only libFuzzer
build stays out of `cargo build --workspace`.

## Requirements

```sh
rustup toolchain install nightly
cargo install cargo-fuzz
```

## Targets

| target        | entry point                        | input   |
| ------------- | ---------------------------------- | ------- |
| `parse`       | `quito_syntax::parse`              | UTF-8   |
| `import_stl`  | `quito_geom::Mesh::from_stl`       | bytes   |
| `import_3mf`  | `quito_geom::Mesh::from_3mf`       | bytes   |
| `import_amf`  | `quito_geom::Mesh::from_amf`       | bytes   |
| `import_off`  | `quito_geom::Mesh::from_off`       | UTF-8   |
| `import_obj`  | `quito_geom::Mesh::from_obj`       | UTF-8   |
| `import_dxf`  | `quito_geom::import_dxf`           | bytes   |
| `import_svg`  | `quito_geom::import_svg`           | bytes   |

The parse-then-**eval** target is intentionally absent: a clean eval fuzzer
needs a global step/fuel budget in the evaluator (nested `for` loops otherwise
multiply the per-construct bounds to an effectively unbounded runtime). That
budget lands in a follow-up PR.

## Running

Seed corpora are reused from the repo rather than duplicated. **The first corpus
directory is where libFuzzer writes newly-found inputs**, so it must be a scratch
dir (`fuzz/corpus/<target>` is gitignored); the repo dirs come after it as
read-only seeds — never pass a committed dir like `corpus/echo` first, or the
fuzzer will write thousands of files into it.

```sh
# from the repo root
cargo +nightly fuzz run parse fuzz/corpus/parse corpus/echo examples benches/models
cargo +nightly fuzz run import_stl fuzz/corpus/import_stl corpus/geom
```

Time-boxed smoke run (what CI does per target):

```sh
cargo +nightly fuzz run parse -- -max_total_time=30 -rss_limit_mb=2048
```

## Reproducing / regressions

A crash writes a reproducer to `fuzz/artifacts/<target>/crash-*`. Replay it:

```sh
cargo +nightly fuzz run <target> fuzz/artifacts/<target>/crash-<hash>
```

Once fixed, commit the reproducer under `fuzz/regressions/<target>/` so it is
replayed on every run (`cargo +nightly fuzz run <target> fuzz/regressions/<target>`).
`corpus/` and `artifacts/` are gitignored; `regressions/` is committed.
