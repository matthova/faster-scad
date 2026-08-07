// UI preferences that persist across sessions but are NOT part of a project, so
// they never enter share links or the autosaved project. Stored under their own
// localStorage key, separate from project.ts.

const KEY = "quito.prefs.v1";

/** Which render engine the playground drives (browser only). "quito" is our own
 *  wasm engine; "openscad" is the vendored OpenSCAD wasm build. */
export type EngineKind = "quito" | "openscad";

/** Render resolution. Presets force `$fn` (so they visibly change a model even
 *  when its script sets its own `$fn`); "normal" injects nothing, so the script
 *  (or the OpenSCAD defaults) decide. "custom" uses the `custom*` fields. */
export type Quality = "draft" | "normal" | "fine" | "custom";

export interface Prefs {
  /** Bidirectional editor↔preview highlighting: code→model (cursor highlights
   *  geometry) and model→code (clicking a face selects its source). */
  linkHighlight: boolean;
  /** Fast preview: unions are concatenated instead of run through the CSG kernel
   *  — much faster to render, but the on-screen mesh is not watertight. Exports
   *  and reported volume/area always use the exact path regardless. */
  fastPreview: boolean;
  /** Active render engine (browser only; the desktop shell always uses native). */
  engine: EngineKind;
  /** Render resolution preset (see Quality). */
  quality: Quality;
  /** Custom-quality overrides; null means "don't inject this one". */
  customFn: number | null;
  customFa: number | null;
  customFs: number | null;
  /** Right-dock collapsed to a spine. null = auto (spine only when no params). */
  dockCollapsed: boolean | null;
  /** Dock section open/closed state. */
  paramsOpen: boolean;
  modelOpen: boolean;
  /** Persisted panel sizes (px). null = use the default. */
  editorWidth: number | null;
  dockWidth: number | null;
  consoleHeight: number | null;
  /** Viewport display toggles (Display ▾ popover). */
  showGrid: boolean;
  showEdges: boolean;
  /** ISO dimension callouts on the bounding box (off by default). */
  showDims: boolean;
  /** Section (clipping) plane: on/off, which axis, and 0..1 position. */
  sectionOn: boolean;
  sectionAxis: "x" | "y" | "z";
  sectionT: number;
  /** Orthographic (vs perspective) camera. Persisted like its Display siblings. */
  ortho: boolean;
  /** App appearance: "auto" follows the OS (prefers-color-scheme); else forced. */
  theme: "auto" | "light" | "dark";
  /** Console drawer: open/closed and the active severity filter. */
  consoleOpen: boolean;
  consoleFilter: "all" | "error" | "warn" | "echo";
  /** The "get the desktop app" callout (browser only) has been dismissed. Sticky
   *  so it doesn't nag on every visit once the user has waved it off. */
  desktopCalloutDismissed: boolean;
}

const DEFAULTS: Prefs = {
  linkHighlight: true,
  fastPreview: false,
  engine: "quito",
  quality: "normal",
  customFn: null,
  customFa: null,
  customFs: null,
  dockCollapsed: null,
  paramsOpen: true,
  modelOpen: true,
  editorWidth: null,
  dockWidth: null,
  consoleHeight: null,
  showGrid: true,
  showEdges: true,
  showDims: false,
  sectionOn: false,
  sectionAxis: "z",
  sectionT: 0.5,
  ortho: false,
  theme: "auto",
  consoleOpen: false,
  consoleFilter: "all",
  desktopCalloutDismissed: false,
};

const THEMES: Prefs["theme"][] = ["auto", "light", "dark"];

const AXES: Array<"x" | "y" | "z"> = ["x", "y", "z"];

const CONSOLE_FILTERS: Prefs["consoleFilter"][] = [
  "all",
  "error",
  "warn",
  "echo",
];

const QUALITIES: Quality[] = ["draft", "normal", "fine", "custom"];

export type QualitySettings = Pick<
  Prefs,
  "quality" | "customFn" | "customFa" | "customFs"
>;

/** The `$fn`/`$fa`/`$fs` overrides (name → literal) a quality setting injects,
 *  in the same shape the customizer uses. "normal" injects nothing. */
export function qualityOverrides(p: QualitySettings): Record<string, string> {
  switch (p.quality) {
    case "draft":
      return { $fn: "16" };
    case "fine":
      return { $fn: "96" };
    case "custom": {
      const o: Record<string, string> = {};
      if (p.customFn != null) o.$fn = String(p.customFn);
      if (p.customFa != null) o.$fa = String(p.customFa);
      if (p.customFs != null) o.$fs = String(p.customFs);
      return o;
    }
    default:
      return {};
  }
}

const num = (v: unknown): number | null =>
  typeof v === "number" && Number.isFinite(v) ? v : null;

export function loadPrefs(): Prefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULTS };
    const p = JSON.parse(raw) as Partial<Prefs>;
    return {
      linkHighlight:
        typeof p.linkHighlight === "boolean"
          ? p.linkHighlight
          : DEFAULTS.linkHighlight,
      fastPreview:
        typeof p.fastPreview === "boolean"
          ? p.fastPreview
          : DEFAULTS.fastPreview,
      engine: p.engine === "openscad" ? "openscad" : DEFAULTS.engine,
      quality: QUALITIES.includes(p.quality as Quality)
        ? (p.quality as Quality)
        : DEFAULTS.quality,
      customFn: num(p.customFn),
      customFa: num(p.customFa),
      customFs: num(p.customFs),
      dockCollapsed:
        typeof p.dockCollapsed === "boolean" ? p.dockCollapsed : null,
      paramsOpen:
        typeof p.paramsOpen === "boolean" ? p.paramsOpen : DEFAULTS.paramsOpen,
      modelOpen:
        typeof p.modelOpen === "boolean" ? p.modelOpen : DEFAULTS.modelOpen,
      editorWidth: num(p.editorWidth),
      dockWidth: num(p.dockWidth),
      consoleHeight: num(p.consoleHeight),
      showGrid:
        typeof p.showGrid === "boolean" ? p.showGrid : DEFAULTS.showGrid,
      showEdges:
        typeof p.showEdges === "boolean" ? p.showEdges : DEFAULTS.showEdges,
      showDims:
        typeof p.showDims === "boolean" ? p.showDims : DEFAULTS.showDims,
      sectionOn:
        typeof p.sectionOn === "boolean" ? p.sectionOn : DEFAULTS.sectionOn,
      sectionAxis: AXES.includes(p.sectionAxis as "x" | "y" | "z")
        ? (p.sectionAxis as "x" | "y" | "z")
        : DEFAULTS.sectionAxis,
      sectionT: num(p.sectionT) ?? DEFAULTS.sectionT,
      ortho: typeof p.ortho === "boolean" ? p.ortho : DEFAULTS.ortho,
      theme: THEMES.includes(p.theme as Prefs["theme"])
        ? (p.theme as Prefs["theme"])
        : DEFAULTS.theme,
      consoleOpen:
        typeof p.consoleOpen === "boolean"
          ? p.consoleOpen
          : DEFAULTS.consoleOpen,
      consoleFilter: CONSOLE_FILTERS.includes(
        p.consoleFilter as Prefs["consoleFilter"],
      )
        ? (p.consoleFilter as Prefs["consoleFilter"])
        : DEFAULTS.consoleFilter,
      desktopCalloutDismissed:
        typeof p.desktopCalloutDismissed === "boolean"
          ? p.desktopCalloutDismissed
          : DEFAULTS.desktopCalloutDismissed,
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
