// UI preferences that persist across sessions but are NOT part of a project, so
// they never enter share links or the autosaved project. Stored under their own
// localStorage key, separate from project.ts.

const KEY = "quito.prefs.v1";

export interface Prefs {
  /** Bidirectional editor↔preview highlighting: code→model (cursor highlights
   *  geometry) and model→code (clicking a face selects its source). */
  linkHighlight: boolean;
}

const DEFAULTS: Prefs = { linkHighlight: true };

export function loadPrefs(): Prefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULTS };
    const p = JSON.parse(raw) as Partial<Prefs>;
    return {
      linkHighlight:
        typeof p.linkHighlight === "boolean" ? p.linkHighlight : DEFAULTS.linkHighlight,
    };
  } catch {
    return { ...DEFAULTS };
  }
}

export function savePrefs(p: Prefs): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    // storage full / unavailable — non-fatal, just don't persist
  }
}
