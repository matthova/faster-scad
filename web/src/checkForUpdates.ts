// In-app auto-update (desktop only). Mirrors the lazy-import pattern in
// `desktopEngine.ts`: the Tauri plugins are imported on demand so the browser
// bundle never evaluates them. The updater checks the signed release manifest
// (see tauri.conf.json `plugins.updater.endpoints`); if a newer version is
// published it downloads the platform artifact, verifies its signature against
// the baked-in public key, installs it, and relaunches.
//
// Note: only the AppImage self-updates on Linux — users who installed the
// .deb/.rpm update through their package manager, and `check()` there is a
// harmless no-op.

/**
 * Check for an update.
 *
 * @param interactive When true (menu-triggered), also report "you're up to
 *   date" and surface errors to the user. When false (silent startup check),
 *   stay quiet unless an update is actually available.
 */
export async function checkForUpdates(interactive = false): Promise<void> {
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();

    if (!update) {
      if (interactive) {
        const { message } = await import("@tauri-apps/plugin-dialog");
        await message("You're running the latest version of Quito.", {
          title: "No updates available",
          kind: "info",
        });
      }
      return;
    }

    const { ask } = await import("@tauri-apps/plugin-dialog");
    const wantsUpdate = await ask(
      `Quito ${update.version} is available (you have ${update.currentVersion}).\n\n` +
        `${update.body ?? ""}\n\nDownload and install it now? ` +
        `The app will restart to finish.`,
      { title: "Update available", kind: "info" },
    );
    if (!wantsUpdate) return;

    // Download + install. The progress callback is where a UI could show a
    // progress bar; for now the OS/webview stays responsive and we simply wait.
    await update.downloadAndInstall();

    // Relaunch into the freshly installed version.
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  } catch (err) {
    if (interactive) {
      const { message } = await import("@tauri-apps/plugin-dialog");
      await message(`Could not check for updates.\n\n${String(err)}`, {
        title: "Update failed",
        kind: "error",
      });
    }
    // Silent startup checks swallow errors (offline, rate-limited, etc.).
  }
}
