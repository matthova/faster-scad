// The single formatter for a KeySpec. Today it produces the human display
// string (used by the palette and the help sheet, killing the hand-typed
// `shortcut` strings and the hand-maintained SHORTCUTS array that had already
// drifted). It also parses a combo into parts so a window-keydown matcher and
// the CodeMirror/Tauri bindings can be generated from the same source as the
// keymap execution migrates onto the registry.
import type { KeySpec } from "./types";

const IS_MAC =
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad/.test(navigator.platform);

const DISPLAY: Record<string, string> = {
  Mod: IS_MAC ? "⌘" : "Ctrl",
  Shift: "⇧",
  Alt: IS_MAC ? "⌥" : "Alt",
  Enter: "↵",
  Escape: "Esc",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
};

/** A combo's parts, lower-cased key last: "Mod-Shift-f" → mods {mod,shift}, key "f". */
export interface ComboParts {
  mod: boolean;
  shift: boolean;
  alt: boolean;
  key: string; // lower-case, e.g. "f", "enter", "k"
}

export function parseCombo(combo: string): ComboParts {
  const parts = combo.split("-");
  const key = parts[parts.length - 1];
  const mods = parts.slice(0, -1);
  return {
    mod: mods.includes("Mod"),
    shift: mods.includes("Shift"),
    alt: mods.includes("Alt"),
    key: key.toLowerCase(),
  };
}

/** Human display, e.g. "⌘⇧F" / "Ctrl+Shift+F". Mac concatenates glyphs; other
 *  platforms join with "+". */
export function displayKey(spec: KeySpec): string {
  const parts = spec.combo.split("-");
  const shown = parts.map((p, i) => {
    if (DISPLAY[p]) return DISPLAY[p];
    // The final segment is the key; single letters upper-case for legibility.
    const isLast = i === parts.length - 1;
    return isLast && p.length === 1 ? p.toUpperCase() : p;
  });
  return IS_MAC ? shown.join("") : shown.join("+");
}

/** Does a DOM keyboard event match this combo? (mod = ⌘ on mac, Ctrl elsewhere) */
export function matchesEvent(spec: KeySpec, e: KeyboardEvent): boolean {
  const p = parseCombo(spec.combo);
  const mod = IS_MAC ? e.metaKey : e.ctrlKey;
  return (
    mod === p.mod &&
    e.shiftKey === p.shift &&
    e.altKey === p.alt &&
    e.key.toLowerCase() === (p.key === "enter" ? "enter" : p.key)
  );
}
