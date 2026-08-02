// UI preferences that persist across sessions but are NOT part of a project, so
// they never enter share links or the autosaved project. Stored under their own
// localStorage key, separate from project.ts.

const KEY = "quito.prefs.v1";

export interface Prefs {
  /** Bidirectional editor↔preview highlighting: code→model (cursor highlights
   *  geometry) and model→code (clicking a face selects its source). */
  linkHighlight: boolean;
  /** Fast preview: unions are concatenated instead of run through the CSG kernel
   *  — much faster to render, but the on-screen mesh is not watertight. Exports
   *  and reported volume/area always use the exact path regardless. */
  fastPreview: boolean;
}

const DEFAULTS: Prefs = { linkHighlight: true, fastPreview: false };

export function loadPrefs(): Prefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULTS };
    const p = JSON.parse(raw) as Partial<Prefs>;
    return {
      linkHighlight:
        typeof p.linkHighlight === "boolean" ? p.linkHighlight : DEFAULTS.linkHighlight,
      fastPreview: typeof p.fastPreview === "boolean" ? p.fastPreview : DEFAULTS.fastPreview,
    };
  } catch {
    return { ...DEFAULTS };
  }
}

/** Persist a partial preference update, merged over what's already stored. */
export function savePrefs(p: Partial<Prefs>): void {
  try {
    localStorage.setItem(KEY, JSON.stringify({ ...loadPrefs(), ...p }));
  } catch {
    // storage full / unavailable — non-fatal, just don't persist
  }
}
