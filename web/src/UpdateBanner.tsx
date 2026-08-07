// The in-app auto-update banner (desktop only). A slim bar below the topbar that
// renders whatever state `useUpdater()` (see `checkForUpdates.ts`) is in — offer,
// download progress, install, and the interactive "up to date"/error results.
// Pure view: no Tauri imports, so it's safe in any bundle.

import { useEffect, useMemo, useState } from "react";
import { renderMarkdown } from "./markdown";
import type { UpdaterState } from "./checkForUpdates";

interface Props {
  state: UpdaterState;
  onInstall: () => void;
  onDismiss: () => void;
}

export function UpdateBanner({ state, onInstall, onDismiss }: Props) {
  const [showNotes, setShowNotes] = useState(false);
  const rawNotes = state.kind === "available" ? state.notes : "";
  const notesHtml = useMemo(
    () => (rawNotes ? renderMarkdown(rawNotes) : ""),
    [rawNotes],
  );

  // The interactive "you're up to date" confirmation self-dismisses so it
  // doesn't linger — it's a transient acknowledgement, not a persistent state.
  useEffect(() => {
    if (state.kind !== "uptodate") return;
    const id = window.setTimeout(onDismiss, 4000);
    return () => window.clearTimeout(id);
  }, [state.kind, onDismiss]);

  if (state.kind === "idle") return null;

  return (
    <div
      className={`update-banner ${state.kind === "error" ? "error" : ""}`}
      role="status"
    >
      <div className="update-banner-row">
        <span className="update-banner-msg">
          {state.kind === "checking" && "Checking for updates…"}
          {state.kind === "available" &&
            `OpenRSCAD ${state.version} is available (you have ${state.currentVersion}).`}
          {state.kind === "downloading" && "Downloading update…"}
          {state.kind === "installing" &&
            "Installing update… the app will restart."}
          {state.kind === "uptodate" &&
            "You're running the latest version of OpenRSCAD."}
          {state.kind === "error" &&
            `Couldn't check for updates: ${state.message}`}
        </span>

        {state.kind === "downloading" && (
          <div className="update-progress">
            <progress
              className="update-progress-bar"
              max={100}
              value={state.pct ?? undefined}
            />
            <span className="update-progress-pct">
              {state.pct == null ? "" : `${state.pct}%`}
            </span>
          </div>
        )}

        <div className="update-banner-actions">
          {state.kind === "available" && state.notes && (
            <button
              className="update-link"
              onClick={() => setShowNotes((s) => !s)}
            >
              {showNotes ? "Hide notes" : "What's new"}
            </button>
          )}
          {state.kind === "available" && (
            <button className="update-primary" onClick={onInstall}>
              Update &amp; Restart
            </button>
          )}
          {/* Everything except the in-flight install/download is dismissible. */}
          {state.kind !== "installing" && state.kind !== "downloading" && (
            <button
              className="update-dismiss"
              onClick={onDismiss}
              aria-label="Dismiss"
            >
              ✕
            </button>
          )}
        </div>
      </div>

      {state.kind === "available" && showNotes && notesHtml && (
        <div
          className="update-notes"
          dangerouslySetInnerHTML={{ __html: notesHtml }}
        />
      )}
    </div>
  );
}
