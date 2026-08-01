# Notes for coding agents

Quito is a clean-room OpenSCAD reimplementation in Rust. Before writing any
code, read [CONTRIBUTING.md](CONTRIBUTING.md) — the **clean-room policy is
mandatory** (never read or paraphrase OpenSCAD's GPL source).

## Commit messages are load-bearing

Releases are cut by **release-please**, which parses Conventional Commits to
decide the next version and to write `CHANGELOG.md`. A malformed subject line
does not fail CI — it silently produces the wrong version or a missing changelog
entry. Get it right at commit time.

Format:

```
type(scope): summary in the imperative mood

Optional body explaining why, wrapped at ~72 chars.

BREAKING CHANGE: only when the public API actually breaks.
```

### Type → effect

| type | changelog section | version bump (0.x) |
| --- | --- | --- |
| `feat` (alias `feature`) | Features | **minor** — 0.1.1 → 0.2.0 |
| `fix` | Bug Fixes | patch — 0.1.1 → 0.1.2 |
| `perf` | Performance | patch |
| `revert` | Reverts | patch |
| `deps` | Dependencies | patch |
| `docs` `refactor` `style` `test` `build` `ci` `chore` | hidden | none on their own |

Two consequences worth internalizing:

- **`feat` costs a minor version.** At 0.x that is the caret boundary for both
  Cargo and npm, so `^0.1.0` consumers of `quito-engine` do not receive it
  automatically. Use `fix` for bug fixes and `feat` only for genuinely new
  capability. Do not reach for `feat` as a default.
- **Hidden types cut no release.** A branch of only `docs:`/`chore:`/`ci:`
  commits produces an empty changelog, and release-please skips the release PR
  entirely ("No user facing commits found"). Mixed with a `feat`/`fix` they ride
  along silently — present in history, absent from the changelog.

Breaking changes use `feat!:` or a `BREAKING CHANGE:` footer. Under 0.x that is
still only a **minor** bump (`bump-minor-pre-major`), not a major.

### Scopes

Optional, but preferred. Existing scopes, roughly by frequency: `web`, `geom`,
`desktop`, `eval`, `lsp`, `npm`, `ci`, `docs`, `release`. Also reasonable:
`syntax`, `ir`, `cli`. Combine with a comma when a change genuinely spans
crates — `feat(geom,web): …`.

### The PR title is the commit message

This repo squash-merges, and its squash title is `COMMIT_OR_PR_TITLE`: with
**one** commit GitHub uses that commit's subject, with **several** it uses the
**PR title**. Either way something you wrote gets parsed by release-please, so
**write the PR title as a valid conventional commit too**. A PR titled
"Fix the thing" squashed from three commits yields an unparseable subject and no
changelog entry.

### Rules

- Never hand-edit a `version` field. release-please owns them across
  `Cargo.toml`, `Cargo.lock`, `desktop/src-tauri/{Cargo.toml,Cargo.lock,tauri.conf.json}`,
  `desktop/{package.json,package-lock.json}`, and
  `packages/npm/{package.json,package-lock.json}`. CI now *asserts* the tag
  matches the tree, so a stray edit fails the release build.
- Never author a `chore(main): release X.Y.Z` commit or PR. That shape is
  release-please's, and hand-writing it confuses its release detection.
- Never **Rebase and merge** the release PR. It is enabled on this repo but it
  strips the PR association release-please uses to find the release commit —
  the release is then skipped with only a log warning. Squash or merge commit.
- To force a version, add a `Release-As: 1.0.0` footer to a commit on `main`.

### Examples

```
feat(geom): support minkowski on non-convex 2D profiles
fix(eval): resolve $children inside nested module instantiation
perf(geom): cache tessellation across warm edits
docs: document the mesh format in the engine README
ci: pin wasm-pack to 0.13 in the publish workflow
```

```
feat(engine)!: return RenderOutput instead of RenderResult

BREAKING CHANGE: `render()` no longer hands back a wasm-owned object,
so callers must stop calling `.free()`.
```

Avoid: `Fix bug`, `update deps`, `WIP`, `feat: various improvements`,
`chore(main): release 0.2.0`.

The full release process, including the human-only steps, is in
[docs/RELEASING.md](docs/RELEASING.md).
