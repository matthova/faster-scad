# Releasing Quito

One tag ships everything: the desktop installers (`desktop/`, Tauri v2 — native
builds for macOS, Windows, and Linux that update themselves in place) and the
`quito-engine` npm package (the wasm build of `crates/quito-wasm`).

Releases are cut by **release-please**, not by tagging by hand.

## How auto-update works

1. On launch (and from **Quito → Check for Updates…**) the app fetches the
   release manifest:
   `https://github.com/matthova/faster-scad/releases/latest/download/latest.json`
2. If the manifest version is newer than the running app, it downloads that
   platform's update artifact, verifies its **minisign signature** against the
   public key baked into `tauri.conf.json`, installs it, and relaunches.
3. `releases/latest` resolves only to a **published, non-prerelease** GitHub
   Release — so a draft or prerelease is invisible to the updater.

Update artifacts per platform:

| OS      | First install     | Self-updates?                        |
| ------- | ----------------- | ------------------------------------ |
| macOS   | `.dmg`            | ✅ (per-arch `.app.tar.gz`)           |
| Windows | NSIS `.exe`       | ✅ (re-runs the signed installer)     |
| Linux   | `.AppImage`       | ✅ AppImage only                      |
| Linux   | `.deb` / `.rpm`   | ❌ update via the package manager     |

## Cutting a release

1. **Land Conventional Commits on `main`** — already the house style
   (`feat(geom,web): …`, `fix(eval,geom): …`). The types and the CHANGELOG
   sections they map to live in `release-please-config.json`;
   `docs`/`chore`/`ci`/`test`/`build`/`refactor`/`style` are hidden and, on their
   own, do **not** cut a release.
2. **release-please keeps a `chore(main): release X.Y.Z` PR open**, updating it as
   commits land. Review the `CHANGELOG.md` diff on it.
   - CI does not start on that PR by itself — it is authored by `GITHUB_TOKEN`,
     so it lands in the approval-required state. Click **Approve workflows to
     run** if you want the checks.
3. **Merge the release PR** with **Squash and merge** or **Create a merge
   commit**. Never **Rebase and merge**: that strips the PR association
   release-please uses to find the release commit, and it will silently skip
   cutting the release with only a log warning.
4. Merging pushes the release commit, creates the tag `vX.Y.Z` and the GitHub
   Release, then dispatches:
   - **Release desktop app** — 4-OS installers + `latest.json`, uploaded onto
     that Release.
   - **Publish engine to npm** — `quito-engine`, via OIDC with provenance.

   Assets appear a few minutes later. Existing desktop users are offered the
   update as soon as `latest.json` uploads.

### Versions are owned by release-please

`.release-please-manifest.json` is the source of truth. release-please writes it
into `Cargo.toml`, `Cargo.lock`, `desktop/src-tauri/{Cargo.toml,Cargo.lock,tauri.conf.json}`,
`desktop/{package.json,package-lock.json}`, and `packages/npm/{package.json,package-lock.json}`.

**Never hand-edit those version fields.** CI no longer stamps versions — it
asserts them, so a tag whose commit carries a different version fails the build
rather than shipping a mismatched installer or a package whose `version()`
disagrees with its own manifest.

Deliberately *not* on the shared version: `web/package.json` (private
playground, never published), `editors/vscode/package.json` (ships to the VS
Code marketplace on its own cadence), and `fuzz/Cargo.toml` (`publish = false`).

### Version bumps

Under 0.x, `bump-minor-pre-major` is on:

| commit | bump | example |
| --- | --- | --- |
| `fix:` | patch | 0.1.1 → 0.1.2 |
| `feat:` | minor | 0.1.1 → 0.2.0 |
| `feat!:` / `BREAKING CHANGE:` | minor | 0.1.1 → 0.2.0 |

To force a specific version, add a `Release-As: 1.0.0` footer to a commit on
`main`.

### Escape hatches

- Re-run a build for an existing tag:
  `gh workflow run release.yml --ref vX.Y.Z -f tag=vX.Y.Z`
  (same for `publish-npm.yml`).
- Publishing a Release **by hand** still fires both workflows via
  `release: published`. Bot-authored releases are ignored on that trigger so the
  dispatch path can't double-fire.

---

## ✅ Human-only checklist

These require secrets, paid accounts, or GitHub UI actions an agent can't do.

### One-time — required for the updater to work at all

- [ ] **Generate the updater signing keypair** (needs the Tauri CLI locally):
      ```sh
      cd desktop && npx tauri signer generate -w ~/.quito-updater.key
      ```
      This prints a **public key** and writes a password-protected **private
      key**. Keep the private key and password secret; they never enter the repo.
- [ ] **Paste the public key** into `desktop/src-tauri/tauri.conf.json` at
      `plugins.updater.pubkey`, replacing `REPLACE_WITH_UPDATER_PUBLIC_KEY`.
      Commit this. (The app will not build a release bundle until this is a real
      key.)
- [ ] **Add repository secrets** (GitHub → Settings → Secrets and variables →
      Actions):
  - [ ] `TAURI_SIGNING_PRIVATE_KEY` — contents of `~/.quito-updater.key`
  - [ ] `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password you chose
- [ ] **Confirm GitHub Actions can create releases**: Settings → Actions →
      General → Workflow permissions → **Read and write permissions**. (The
      workflow also requests `contents: write`.)
- [ ] **Allow Actions to open PRs**: Settings → Actions → General → **Allow
      GitHub Actions to create and approve pull requests**. Without it
      release-please cannot open its release PR.
- [ ] **Register the npm trusted publisher** (one-time; `quito-engine@0.0.0`
      already exists on the registry, so only the registration is left):
      npmjs.com → `quito-engine` → Settings → Trusted Publisher → GitHub
      Actions, with Organization or user `matthova`, Repository `faster-scad`,
      Workflow filename `publish-npm.yml`, Environment blank. Fields are
      case-sensitive and exact, and npm does not validate them at save time.

### Per release

- [ ] **Merge the `chore(main): release X.Y.Z` PR** (Squash or Merge commit — never
      Rebase). Everything downstream is automatic. Merge only when ready to
      ship: existing desktop users are offered the update as soon as the assets
      upload.

### Recommended before a public launch — OS code-signing

Without these, users get "unidentified developer" / SmartScreen warnings on
first install (auto-update still works). Signing must be enabled in the release
workflow (`.github/workflows/release.yml` has the macOS env block commented in).

- [ ] **macOS**: enroll in the Apple Developer Program ($99/yr); create a
      "Developer ID Application" certificate; add secrets `APPLE_CERTIFICATE`,
      `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
      `APPLE_PASSWORD` (app-specific password), `APPLE_TEAM_ID`; uncomment the
      Apple env block in the workflow. This also enables **notarization**.
- [ ] **Windows**: obtain a code-signing certificate (Azure Trusted Signing is
      the cheapest modern option; an OV/EV cert also works) and wire it into the
      Windows bundle config / workflow.
- [ ] **Linux**: no signing required.

### Optional / nice-to-have

- [x] Decide whether Linux users should be steered to the **AppImage** (the only
      self-updating Linux format) vs. `.deb`/`.rpm` in the README/download page.
      The README download table links the AppImage; `.deb`/`.rpm` stay reachable
      via the "Browse all downloads" release-page link.
- [x] Add a download/landing page linking to the latest release assets — the
      README's **Download** table links each platform via stable, version-less
      asset aliases (`Quito-<platform>...`) uploaded by the release workflow, so
      `releases/latest/download/<name>` always resolves to the newest build.
- [ ] Add an in-app download/callout in the **web playground** (`web/src/App.tsx`,
      gated to the non-Tauri build) linking to the desktop downloads.
