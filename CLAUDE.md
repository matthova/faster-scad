# Notes for coding agents

OpenRSCAD is a clean-room OpenSCAD reimplementation in Rust. Before writing any
code, read [CONTRIBUTING.md](CONTRIBUTING.md) — the **clean-room policy is
mandatory** (never read or paraphrase OpenSCAD's GPL source).

## Changesets are load-bearing

Releases are cut by **[changesets](https://github.com/changesets/changesets)**,
not by parsing commit messages. Versioning is driven by `.changeset/*.md` files
you add **in the same PR as your change**. A user-facing change that ships with
no changeset does not fail CI — it silently lands with no changelog entry and no
version bump. Get it right at PR time.

### Add a changeset

From the repo root:

```
npx changeset
```

Pick the bump, write a one-line **user-facing** summary. It writes a
`.changeset/<name>.md` — commit it with your change. (You can also hand-write the
file; see the format in existing ones / `.changeset/README.md`.) The summary is
what lands verbatim in `CHANGELOG.md` and the GitHub Release notes, so write it
for a reader, not a commit log.

### Picking the bump (we are pre-1.0)

The whole repo shares **one** version, so the bump is repo-wide, not per-crate.

| bump | pre-1.0 effect | use for |
| --- | --- | --- |
| `patch` | 0.2.0 → 0.2.1 | bug fixes, perf, dependency bumps |
| `minor` | 0.2.0 → 0.3.0 | genuinely new capability — **and** any breaking change |
| `major` | → 1.0.0 | do not use until we deliberately cut 1.0 |

Two consequences worth internalizing:

- **`minor` costs the caret boundary.** At 0.x, `0.2.0 → 0.3.0` is the caret
  boundary for both Cargo and npm, so `^0.2.0` consumers of `openrscad-engine` do not
  receive it automatically. Reach for `patch` by default and `minor` only for
  genuinely new or breaking behavior.
- **Breaking changes are still `minor` pre-1.0.** Select `minor` and call the
  break out in the summary. Do not select `major` — that jumps us to 1.0.0.
- **No changeset = no release entry.** A PR of pure `docs`/`ci`/`test`/`refactor`
  churn needs none. But a real fix or feature with no changeset rides in silently:
  present in history, absent from the changelog, and it won't move the version.

### Commit messages (no longer load-bearing)

Commit messages and PR titles no longer drive versioning, so a malformed subject
can't produce the wrong bump. Keep the existing `type(scope): summary` house
style anyway for a readable history — it's just no longer parsed. Scopes in use,
roughly by frequency: `web`, `geom`, `desktop`, `eval`, `lsp`, `npm`, `ci`,
`docs`, `release`; also `syntax`, `ir`, `cli`.

### Rules

- Never hand-edit a `version` field. `scripts/sync-versions.mjs` (run by
  `npm run version`) owns them across `package.json` (root, canonical),
  `Cargo.toml`, `Cargo.lock`,
  `desktop/src-tauri/{Cargo.toml,Cargo.lock,tauri.conf.json}`,
  `desktop/{package.json,package-lock.json}`, and
  `packages/npm/{package.json,package-lock.json}`. CI *asserts* the tag matches
  the tree, so a stray edit fails the release build.
- Never hand-run `changeset version` on a feature branch, and never hand-author a
  `chore: version packages` commit or PR — that shape is the changesets action's.
- The "Version Packages" PR keys the release on the committed version, not on any
  PR association, so **any** merge style (squash, merge commit, or rebase) is
  safe for it.
- To force a specific version, add an empty `major`/`minor`/`patch` changeset (or
  edit the pending one) so the bump lands where you want it.

### Example changeset

`.changeset/curly-lions-cheer.md`:

```
---
"openrscad-release-root": minor
---

support minkowski on non-convex 2D profiles
```

The full release process, including the human-only steps, is in
[docs/RELEASING.md](docs/RELEASING.md).
