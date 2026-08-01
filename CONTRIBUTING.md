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

## Changesets

Quito versions and releases with
[changesets](https://github.com/changesets/changesets). Add a changeset **in the
same PR** as any user-facing change:

```sh
npx changeset
```

Pick the bump, write a one-line user-facing summary, commit the resulting
`.changeset/*.md`. A change that ships with no changeset doesn't fail CI — it
just lands with no changelog entry and no version bump. Pure
`docs`/`ci`/`test`/`refactor` churn needs no changeset.

The repo shares **one** version, so the bump is repo-wide:

| bump | pre-1.0 effect | use for |
| --- | --- | --- |
| `patch` | 0.2.0 → 0.2.1 | bug fixes, perf, dependency bumps |
| `minor` | 0.2.0 → 0.3.0 | genuinely new capability — **and** any breaking change |
| `major` | → 1.0.0 | do not use until we deliberately cut 1.0 |

`minor` crosses the caret boundary (`^0.2.0` consumers of `quito-engine` won't
pick it up automatically), so reach for `patch` by default. Breaking changes are
still `minor` while we're 0.x — select `minor` and call the break out in the
summary.

Commit messages and PR titles no longer drive releases; keep the `type(scope):`
house style for readable history anyway (scopes in use: `web`, `geom`,
`desktop`, `eval`, `lsp`, `npm`, `ci`, `docs`, `release`, plus `syntax` / `ir` /
`cli`).

Never hand-edit a `version` field — `scripts/sync-versions.mjs` owns all of them
and CI asserts tag and tree agree. See [docs/RELEASING.md](docs/RELEASING.md) for
the full process and `.changeset/README.md` for the changeset format.

## Formatting

CI enforces `cargo fmt --all --check`. A repo-tracked pre-commit hook in
`.githooks/` auto-formats staged Rust files so commits stay clean. Enable it once
per clone (it applies to all git worktrees too):

```sh
git config core.hooksPath .githooks
```

Formatting still uses the standard tools directly — run `cargo fmt --all` anytime,
and `cargo fmt --all --check` to verify.
