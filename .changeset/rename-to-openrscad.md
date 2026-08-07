---
"openrscad-release-root": minor
---

Rename the project from Quito to OpenRSCAD. This renames the published npm engine
package (`quito-engine` → `openrscad-engine`), the Rust crates (`quito-*` →
`openrscad-*`), the desktop app and its bundle identifier, and all user-facing
branding. **Breaking:** consumers of `quito-engine` must switch to
`openrscad-engine`, and desktop users get a fresh app identity (previous
auto-updates do not carry over across the new bundle identifier).
