// Help / shortcut sheet: a modal that surfaces the app's keyboard shortcuts and
// the features that are otherwise undiscoverable (nav cube, $vp camera scripting,
// BOSL2 auto-fetch, the !/#/%/* modifier characters). Opened from the ? button.
import { useEffect } from "react";

interface Props {
  onClose: () => void;
}

const MOD =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform)
    ? "⌘"
    : "Ctrl";

const SHORTCUTS: [string, string][] = [
  [`${MOD}K`, "Command palette"],
  [`${MOD}↵`, "Render"],
  [`${MOD}J`, "Toggle console"],
  [`${MOD}⇧F`, "Zoom to fit"],
  [`${MOD}S`, "Save file (desktop)"],
  ["Esc", "Dismiss the editor↔preview highlight"],
  ["Tab", "Indent (in the editor)"],
];

const TIPS: [string, string][] = [
  [
    "Navigation cube",
    "Drag it to orbit; click a face, edge, or corner to fly to that view (all 7 presets).",
  ],
  [
    "Zoom to fit",
    "The Fit button (by the cube) frames the model without changing the angle.",
  ],
  [
    "Camera scripting",
    "Read $vpr / $vpt / $vpd / $vpf in your script to drive the camera from code; the live camera is fed back in.",
  ],
  [
    "BOSL2",
    "include <BOSL2/std.scad> auto-fetches the library from a CDN on first use — no install.",
  ],
  [
    "Modifier characters",
    "Prefix a statement with ! (show only), # (highlight), % (background/transparent), or * (disable).",
  ],
  [
    "Dimensions",
    "Display ▾ → Dimensions draws ISO width/depth/height callouts that also appear in PNG captures.",
  ],
  [
    "Sharing",
    "Share copies a link that encodes the whole project (and the current $t frame) in the URL.",
  ],
];

export function HelpSheet({ onClose }: Props) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  return (
    <div className="help-overlay" onPointerDown={onClose}>
      <div
        className="help"
        role="dialog"
        aria-label="Help"
        onPointerDown={(e) => e.stopPropagation()}
      >
        <div className="help-head">
          <span>Quito playground — help</span>
          <button className="help-close" onClick={onClose} aria-label="Close">
            ✕
          </button>
        </div>
        <div className="help-body">
          <section>
            <h3>Keyboard</h3>
            <dl className="help-keys">
              {SHORTCUTS.map(([k, v]) => (
                <div key={k}>
                  <dt>
                    <kbd>{k}</kbd>
                  </dt>
                  <dd>{v}</dd>
                </div>
              ))}
            </dl>
          </section>
          <section>
            <h3>Things you might miss</h3>
            <dl className="help-tips">
              {TIPS.map(([k, v]) => (
                <div key={k}>
                  <dt>{k}</dt>
                  <dd>{v}</dd>
                </div>
              ))}
            </dl>
          </section>
        </div>
      </div>
    </div>
  );
}
