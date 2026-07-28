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

1. Bump the version in **all three** files (keep them identical):
   - `desktop/src-tauri/tauri.conf.json` → `version`
   - `desktop/package.json` → `version`
   - `desktop/src-tauri/Cargo.toml` → `version`
   Then `cargo build --manifest-path desktop/src-tauri/Cargo.toml` to refresh
   `desktop/src-tauri/Cargo.lock`, and commit.
2. Run the release workflow **manually** (it does not trigger on tag push):
   - GitHub → **Actions** → **Release desktop app** → **Run workflow**, and
     enter the version tag (e.g. `v0.1.0`, must match `tauri.conf.json`), or
   - from the CLI: `gh workflow run "Release desktop app" -f tag=v0.1.0`

   The workflow creates the tag for you as part of drafting the release.
3. The **Release desktop app** workflow builds + signs on all three OSes and
   creates a **draft** GitHub Release with the installers and `latest.json`.
4. Review the draft release, then **Publish** it. Publishing is what exposes the
   update to existing users — do it only when you're ready to ship.

> The updater's `version` comparison uses `tauri.conf.json`. The Git tag and the
> three version fields must all agree, or users won't be offered the update.

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

- [ ] After the workflow finishes, **publish the draft release** (and make sure
      "Set as a pre-release" is **unchecked**), or the updater won't see it.

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
