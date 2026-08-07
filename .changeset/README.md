# Changesets

This folder is managed by [changesets](https://github.com/changesets/changesets),
which drives OpenRSCAD's versioning and releases (it replaced release-please).

## What you do

When your change should ship in a release, add a changeset **in the same PR**:

```sh
npx changeset
```

Pick the bump and write a one-line, user-facing summary. It creates a
`.changeset/<name>.md` file — commit it. Changes with no user-facing effect
(pure `docs`/`ci`/`test`/`refactor` churn) need no changeset.

## Picking the bump (we are pre-1.0)

The whole repo shares one version, so the bump is repo-wide.

| bump    | pre-1.0 effect  | use for                                            |
| ------- | --------------- | -------------------------------------------------- |
| `patch` | 0.2.0 → 0.2.1   | bug fixes, perf, dependency bumps                  |
| `minor` | 0.2.0 → 0.3.0   | new capability — **and** any breaking API change   |
| `major` | → 1.0.0         | do not use before we deliberately cut 1.0          |

Under 0.x a breaking change is still a **minor** by our policy — select `minor`,
and call the break out in the summary. `minor` crosses the caret boundary
(`^0.2.0` consumers of `openrscad-engine` don't auto-receive it), so reach for
`patch` by default and `minor` only for genuinely new or breaking behavior.

The `.changeset/*.md` summaries are what land in `CHANGELOG.md` and the GitHub
Release notes, so write them for a reader, not a commit log.

See [docs/RELEASING.md](../docs/RELEASING.md) for the full flow.
