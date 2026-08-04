// In-app auto-update (desktop only). Mirrors the lazy-import pattern in
// `desktopEngine.ts`: the Tauri plugins are imported on demand so the browser
// bundle never evaluates them. The updater checks the signed release manifest
// (see tauri.conf.json `plugins.updater.endpoints`); if a newer version is
// published it downloads the platform artifact, verifies its signature against
// the baked-in public key, installs it, and relaunches.
//
// The lifecycle is surfaced through an in-app React banner (`UpdateBanner`)
// rather than native OS dialogs: `useUpdater()` owns the state machine and the
// download-progress percentage, the banner renders it. Silent startup checks
// stay invisible unless an update is actually available; interactive checks
// (the "Check for Updates…" menu item) also report "up to date" and errors.
//
// Note: only the AppImage self-updates on Linux — users who installed the
// .deb/.rpm update through their package manager, and `check()` there is a
// harmless no-op.

import { useCallback, useRef, useState } from "react";

/** The live `Update` handle returned by the updater plugin's `check()`. */
type UpdateHandle = {
  version: string;
  currentVersion: string;
  body?: string;
  downloadAndInstall: (
    onEvent?: (event: DownloadEvent) => void,
  ) => Promise<void>;
};

/** Progress events emitted by `downloadAndInstall`. */
type DownloadEvent =
  | { event: "Started"; data: { contentLength?: number } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" };

/**
 * The updater's observable state. `idle` hides the banner; every other state
 * renders it. `uptodate`/`error` are only ever entered by interactive checks —
 * silent startup checks fall back to `idle` so they stay invisible.
 */
export type UpdaterState =
  | { kind: "idle" }
  | { kind: "checking" }
  | {
      kind: "available";
      version: string;
      currentVersion: string;
      notes: string;
    }
  | { kind: "downloading"; version: string; pct: number | null }
  | { kind: "installing"; version: string }
  | { kind: "uptodate" }
  | { kind: "error"; message: string };

export interface Updater {
  state: UpdaterState;
  /** Check for an update. `interactive` surfaces "up to date"/errors in the UI. */
  check: (interactive: boolean) => Promise<void>;
  /** Download + install the available update, then relaunch. */
  startInstall: () => Promise<void>;
  /** Dismiss the banner (back to idle). */
  dismiss: () => void;
}

/**
 * Percentage (0–100, rounded) of `downloaded` out of `total` bytes, or `null`
 * when the total is unknown (the server didn't send a Content-Length) — the
 * banner renders that as an indeterminate bar. Pure: unit-testable without Tauri.
 */
export function progressPct(
  downloaded: number,
  total: number | null,
): number | null {
  if (!total || total <= 0) return null;
  return Math.min(100, Math.round((downloaded / total) * 100));
}

/**
 * React controller for the desktop auto-updater. Inert until `check()` runs, so
 * it's safe to call unconditionally from a component that also renders in the
 * browser (the Tauri plugins are only imported once a check actually starts).
 */
export function useUpdater(): Updater {
  const [state, setState] = useState<UpdaterState>({ kind: "idle" });
  // The live update handle is held outside React state — it isn't renderable and
  // must survive re-renders so `startInstall()` can act on the checked update.
  const handleRef = useRef<UpdateHandle | null>(null);

  const check = useCallback(async (interactive: boolean) => {
    if (interactive) setState({ kind: "checking" });
    try {
      const { check: runCheck } = await import("@tauri-apps/plugin-updater");
      const update = (await runCheck()) as UpdateHandle | null;
      if (!update) {
        handleRef.current = null;
        // Silent checks disappear; interactive checks confirm "you're current".
        setState(interactive ? { kind: "uptodate" } : { kind: "idle" });
        return;
      }
      handleRef.current = update;
      setState({
        kind: "available",
        version: update.version,
        currentVersion: update.currentVersion,
        notes: update.body ?? "",
      });
    } catch (err) {
      handleRef.current = null;
      // Silent startup checks swallow errors (offline, rate-limited, etc.).
      setState(
        interactive
          ? { kind: "error", message: String(err) }
          : { kind: "idle" },
      );
    }
  }, []);

  const startInstall = useCallback(async () => {
    const update = handleRef.current;
    if (!update) return;
    const version = update.version;
    setState({ kind: "downloading", version, pct: null });
    try {
      let total: number | null = null;
      let downloaded = 0;
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? null;
            downloaded = 0;
            setState({
              kind: "downloading",
              version,
              pct: progressPct(0, total),
            });
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            setState({
              kind: "downloading",
              version,
              pct: progressPct(downloaded, total),
            });
            break;
          case "Finished":
            setState({ kind: "installing", version });
            break;
        }
      });
      // Relaunch into the freshly installed version.
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (err) {
      setState({ kind: "error", message: String(err) });
    }
  }, []);

  const dismiss = useCallback(() => setState({ kind: "idle" }), []);

  return { state, check, startInstall, dismiss };
}
