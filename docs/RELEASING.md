# Releasing the Quito desktop app

The desktop app (`desktop/`, Tauri v2) ships as native installers for macOS,
Windows, and Linux, and updates itself in place via the Tauri updater.

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

## Cutting a release (repeat each version)

The release **tag** is the single source of truth for the version — you do
**not** bump any version fields in the repo to cut a release.

1. **Publish a GitHub Release** with a tag of `v<version>` (e.g. `v0.1.1`):
   - GitHub → **Releases** → **Draft a new release** → **Choose a tag** →
     type the new tag (it's created on publish) → **Publish release**, or
   - from the CLI: `gh release create v0.1.1 --title "v0.1.1" --generate-notes`

   Publishing fires the **Release desktop app** workflow automatically.
2. The workflow stamps the tag into `tauri.conf.json`'s `version` (`v0.1.1` →
   `0.1.1`, in the CI checkout only — nothing is committed back), then builds +
   signs on all three OSes and **uploads the installers and `latest.json` as
   assets onto the release you just published**. They appear a few minutes later.
3. That's it — the release is already published, so existing users are offered
   the update as soon as the assets finish uploading. (Don't publish until
   you're ready to ship. To stage instead, mark it a **pre-release**: the
   updater ignores pre-releases, and you can un-check that later.)

> The version shown in installers and used by the updater's `version` comparison
> comes entirely from the tag. The `version` fields committed in
> `tauri.conf.json` / `package.json` / `Cargo.toml` only matter for local
> `tauri dev` — they don't need to match the release tag. Keep them roughly
> current if you like, but it's not required to ship.

You can also re-run a build against an existing release via **Actions →
Release desktop app → Run workflow** (or `gh workflow run "Release desktop app"
-f tag=v0.1.1`).

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

### Per release

- [ ] **Publish the GitHub Release** (with "Set as a pre-release" **unchecked**)
      to trigger the build. This exposes the update to existing users once the
      assets finish uploading, so publish only when ready to ship.

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

- [ ] Decide whether Linux users should be steered to the **AppImage** (the only
      self-updating Linux format) vs. `.deb`/`.rpm` in the README/download page.
- [ ] Add a download/landing page linking to the latest release assets.
