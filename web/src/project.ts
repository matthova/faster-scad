// Project persistence: the current files, parameter overrides, and active tab
// are autosaved to localStorage and restored on load, so a reload (or revisit)
// keeps your work.
import type { ParamValue } from "./customizer";

export interface File {
  name: string;
  content: string;
  /** Absolute disk path (desktop only); set once a file is opened or saved to
   *  disk. Browser files never have one, so it never enters share links. */
  path?: string;
}

export interface Project {
  files: File[];
  overrides: Record<string, ParamValue>;
  active: number;
  /** Saved customizer parameter sets (named presets of override values). */
  paramSets?: Record<string, Record<string, ParamValue>>;
}

const KEY = "quito.project.v1";

export function saveProject(p: Project): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    // storage full / unavailable — non-fatal, just don't persist
  }
}

export function loadProject(): Project | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const p = JSON.parse(raw) as Project;
    if (!Array.isArray(p.files) || p.files.length === 0) return null;
    // Validate shape defensively.
    if (!p.files.every((f) => typeof f.name === "string" && typeof f.content === "string"))
      return null;
    return {
      files: p.files,
      overrides: p.overrides && typeof p.overrides === "object" ? p.overrides : {},
      active: Number.isInteger(p.active) ? Math.min(Math.max(0, p.active), p.files.length - 1) : 0,
      paramSets: p.paramSets && typeof p.paramSets === "object" ? p.paramSets : {},
    };
  } catch {
    return null;
  }
}

export function clearProject(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // ignore
  }
}
