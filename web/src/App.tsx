import { useEffect, useRef, useState } from "react";
import { EditorView, keymap } from "@codemirror/view";
import { EditorState, Compartment, Prec } from "@codemirror/state";
import { syntaxHighlighting } from "@codemirror/language";
import { setDiagnostics, type Diagnostic } from "@codemirror/lint";
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { openscad } from "./lang/openscad";
import {
  darkTheme,
  lightTheme,
  darkHighlight,
  lightHighlight,
} from "./lang/theme";
import {
  Viewer,
  type MeshInfo,
  type PreviewGroup,
  type ProvenanceGroup,
  type ThemeMode,
  type Span,
} from "./viewer";
import {
  Engine,
  OpenscadEngine,
  export2dBrowser,
  renderMeshExactBrowser,
} from "./engine";
import type { RenderResponse } from "./engineWorker";
import {
  buildBinarySTL,
  buildOFF,
  buildOBJ,
  build3MF,
  build3MFColored,
  buildAMF,
  downloadBlob,
  zipFiles,
} from "./stl";
import { Dock } from "./Dock";
import { ResizeHandle } from "./ResizeHandle";
import { Popover, PopoverToggle } from "./Popover";
import { CommandPalette, type Command } from "./CommandPalette";
import { HelpSheet } from "./HelpSheet";
import {
  parseSchema,
  toLiteral,
  toParamSetsJson,
  fromParamSetsJson,
  type Param,
  type ParamValue,
} from "./customizer";
import {
  loadProject,
  saveProject,
  clearProject,
  markRenderPending,
  settleRenderPending,
  wasRenderPending,
  type File,
} from "./project";
import {
  loadPrefs,
  savePrefs,
  qualityOverrides,
  type EngineKind,
  type Quality,
  type QualitySettings,
} from "./prefs";
import { EXAMPLES } from "./examples";
import { decodeSharedProject, shareUrl } from "./share";
import { resolveClosure } from "./library";
import {
  isTauri,
  openExternal,
  DesktopEngine,
  DesktopOpenscadEngine,
  saveModelNative,
  saveBytesNative,
  openScadFile,
  openScadPath,
  takePendingOpen,
  onFileChanged,
  saveSource,
  saveSourceAs,
  saveImageNative,
  watchFiles,
  onOpenPath,
  onMenuAction,
} from "./desktopEngine";
import { useUpdater } from "./checkForUpdates";
import { UpdateBanner } from "./UpdateBanner";

const TAURI = isTauri();

const GITHUB_URL = "https://github.com/matthova/faster-scad";

// Base URL for bundled libraries (public/lib/…), resolved against the page.
const LIB_BASE = new URL("lib/", document.baseURI).href;

/** The current OS appearance from `prefers-color-scheme`. */
function currentMode(): ThemeMode {
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

/** Editor theme + syntax-highlighting extensions for a given appearance. Placed
 *  after `basicSetup` so this style beats its `{fallback:true}` default. */
function themeExts(mode: ThemeMode) {
  return mode === "dark"
    ? [darkTheme, syntaxHighlighting(darkHighlight)]
    : [lightTheme, syntaxHighlighting(lightHighlight)];
}

// The first file is always the rendered "main"; the rest are libraries that
// main can `use`/`include`.
const DEFAULT_FILES: File[] = [
  {
    name: "main.scad",
    content: `// Quito playground — edits re-render live.
// main.scad uses helpers.scad (see the tab); tweak the parameters at right.
use <helpers.scad>
$fn = 48;

/* [Box] */
size = 30;    // [10:60]
radius = 4;   // [1:12]

/* [Lid] */
lid = true;
lid_gap = 1;  // [0:0.5:4]

rounded_box([size, size, size], radius);
if (lid)
  translate([0, 0, size/2 + lid_gap + radius])
    rounded_box([size, size, 4], radius);

echo("box size", size, "radius", radius);
`,
  },
  {
    name: "helpers.scad",
    content: `// A tiny helper library, used by main.scad.
module rounded_box(sz, r) {
  minkowski() {
    cube([sz[0] - 2*r, sz[1] - 2*r, sz[2] - 2*r], center = true);
    sphere(r);
  }
}
`,
  },
];

type ExportFmt = "stl" | "off" | "obj" | "3mf" | "amf" | "dxf" | "svg";

/** Format a bounding-box dimension: whole numbers plain, else up to 2 decimals. */
function fmtDim(v: number): string {
  return Number.isInteger(v) ? String(v) : v.toFixed(2).replace(/\.?0+$/, "");
}

/** Last path segment (handles both `/` and `\` separators). */
function basename(path: string): string {
  const seg = path.split(/[/\\]/).pop();
  return seg && seg.length ? seg : path;
}

/** A structured diagnostic from the engine (byte offsets into the main source). */
interface EngineDiag {
  severity: "error" | "warning";
  message: string;
  start: number; // UTF-8 byte offset, or -1 when unknown
  end: number;
}

/** UTF-8 byte length of a Unicode code point. */
function utf8Len(cp: number): number {
  return cp <= 0x7f ? 1 : cp <= 0x7ff ? 2 : cp <= 0xffff ? 3 : 4;
}

/** Map a UTF-8 byte offset (engine spans) to a UTF-16 index (CodeMirror), since
 *  the engine measures the source as UTF-8 bytes but JS strings are UTF-16. */
function byteToChar(source: string, byte: number): number {
  if (byte <= 0) return 0;
  let b = 0;
  let i = 0;
  while (i < source.length) {
    if (b >= byte) return i;
    const cp = source.codePointAt(i)!;
    b += utf8Len(cp);
    i += cp > 0xffff ? 2 : 1;
  }
  return source.length;
}

/** Map a UTF-16 index (CodeMirror positions) to a UTF-8 byte offset (engine
 *  spans) — the inverse of {@link byteToChar}, for resolving the editor cursor
 *  against provenance spans. */
function charToByte(source: string, char: number): number {
  if (char <= 0) return 0;
  let b = 0;
  let i = 0;
  while (i < source.length && i < char) {
    const cp = source.codePointAt(i)!;
    b += utf8Len(cp);
    i += cp > 0xffff ? 2 : 1;
  }
  return b;
}

/** Convert engine diagnostics (with byte spans) to CodeMirror lint diagnostics,
 *  mapped against `source` (the main file). Entries without a span are dropped
 *  (they still show in the console). */
function toCmDiagnostics(diags: EngineDiag[], source: string): Diagnostic[] {
  const out: Diagnostic[] = [];
  for (const d of diags) {
    if (d.start < 0 || d.end < 0) continue;
    let from = byteToChar(source, d.start);
    let to = byteToChar(source, d.end);
    if (to < from) to = from;
    if (to === from) to = Math.min(source.length, from + 1); // widen a point marker
    out.push({ from, to, severity: d.severity, message: d.message });
  }
  return out;
}

// Formats offered per model dimensionality — 2D profiles export to vector
// formats, 3D solids to mesh formats.
const FORMATS_3D: ExportFmt[] = ["stl", "off", "obj", "3mf", "amf"];
const FORMATS_2D: ExportFmt[] = ["dxf", "svg"];

// A model is "multi-color" for export purposes when its exportable (non-`%`
// background) color groups use more than one distinct color. STL silently drops
// color; 3MF preserves it as separate objects — so we default multi-color models
// to 3MF until the user picks a format themselves.
function distinctExportColors(groups: PreviewGroup[]): number {
  const seen = new Set<string>();
  for (const g of groups) {
    if (g.mode === "background") continue;
    seen.add(g.color.join(","));
  }
  return seen.size;
}

// A render running longer than this is considered a death-spiral candidate and
// arms the crash-recovery sentinel (see project.ts). Fast renders never trip it,
// so an ordinary reload mid-render doesn't force recovery mode on next load.
const SLOW_RENDER_MS = 3000;

// Panel-size defaults + bounds (px) for the resizable layout.
const EDITOR_W_DEFAULT = 460;
const DOCK_W_DEFAULT = 288;
const CONSOLE_H_DEFAULT = 160;
const clampNum = (v: number, lo: number, hi: number) =>
  Math.max(lo, Math.min(hi, v));

interface Status {
  ok: boolean;
  message: string;
  triangleCount: number;
  vertexCount: number;
  /** Total surface area (3D) or enclosed area (2D). */
  area: number;
  volume: number;
  ms: number;
  echo: string;
  warnings: string;
  error: string;
  /** Recoverable geometry errors (degraded render): a mesh is shown but a CSG op
   *  failed and was replaced by a fallback. Empty when geometry is exact. */
  geomErrors: string;
  /** The shown mesh came from the fast, non-watertight preview path, so `volume`
   *  is approximate (it counts skipped-union interior walls). */
  preview: boolean;
}

export function App() {
  const editorHost = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewerRef = useRef<Viewer | null>(null);
  const engineRef = useRef<
    Engine | DesktopEngine | DesktopOpenscadEngine | null
  >(null);
  // Directory of the opened file, for the native engine's disk include/use
  // resolution. Held here (not just on the engine instance) so it survives an
  // engine swap: switching to OpenSCAD and back rebuilds the DesktopEngine, and
  // it needs to be re-seeded with the current file's dir.
  const engineDirRef = useRef(".");
  const viewRef = useRef<EditorView | null>(null);
  const lastPositions = useRef<Float32Array>(new Float32Array(0));
  // Preview color channel from the last render, for colored 3MF export.
  const lastPreview = useRef<{
    positions: Float32Array;
    groups: PreviewGroup[];
  }>({
    positions: new Float32Array(0),
    groups: [],
  });
  const debounceTimer = useRef<number | undefined>(undefined);
  // Crash-recovery: a render still running after SLOW_RENDER_MS arms the sentinel
  // so a force-reload during a hang recovers instead of re-triggering the freeze.
  const slowTimer = useRef<number | undefined>(undefined);
  // Editor theme lives in a Compartment so it can be reconfigured live when the
  // OS appearance flips, without recreating the editor.
  const themeComp = useRef(new Compartment());
  // Provenance groups from the last render, for editor↔preview linking (the
  // viewer owns the pick geometry; this resolves the cursor → span, code→model).
  const provenanceRef = useRef<ProvenanceGroup[]>([]);
  const highlightFromCursorRef = useRef<() => void>(() => {});
  // A click on empty preview space (or Escape) dismisses the highlight; it stays
  // cleared until the next cursor move / item click so a re-render (which re-runs
  // the cursor→model highlight) doesn't resurrect it.
  const highlightDismissedRef = useRef(false);
  // Editor↔preview highlighting toggle. Mirrored to a ref so the once-wired pick
  // handler (in useEffect) reads the live value, not a stale closure.
  const linkHighlightRef = useRef(loadPrefs().linkHighlight);
  // Fast (non-watertight) preview toggle. Mirrored to a ref so the once-wired
  // `renderNow` closure reads the live value.
  const fastPreviewRef = useRef(loadPrefs().fastPreview);
  // Render quality ($fn/$fa/$fs). Mirrored to a ref so `renderNow` injects the
  // live setting; a NOT-in-share-link pref (quality is a viewing preference).
  const qualityRef = useRef<QualitySettings>({
    quality: loadPrefs().quality,
    customFn: loadPrefs().customFn,
    customFa: loadPrefs().customFa,
    customFs: loadPrefs().customFs,
  });
  // Active render engine. "quito" is our engine (native C++ kernel on desktop,
  // wasm in the browser); "openscad" is the vendored OpenSCAD wasm build, which
  // runs in-webview on both. Mirrored to a ref so the once-wired render closures
  // read the live value. `swapEngineRef` is set inside the mount effect (it needs
  // the effect's onResult/onBusyChange).
  const engineKindRef = useRef<EngineKind>(loadPrefs().engine);
  const swapEngineRef = useRef<(kind: EngineKind) => void>(() => {});

  // File + customizer state. A `#code/…` share link (browser only) wins over
  // the autosaved localStorage project, so opening a shared URL always shows
  // that project. Refs mirror state so imperative render/edit paths never see a
  // stale closure.
  const sharedRef = useRef(TAURI ? null : decodeSharedProject());
  const saved = useRef(sharedRef.current ?? loadProject()).current;
  // Death-spiral recovery: if the previous session left a render in flight (it
  // froze/crashed on too-heavy geometry), don't auto-render the restored project
  // on load — that just re-triggers the freeze. A share link is fresh, chosen
  // content, so it always renders. Read once at startup, before any render arms
  // the sentinel again.
  const wasStuck = useRef(!sharedRef.current && wasRenderPending()).current;
  const filesRef = useRef<File[]>(
    saved?.files ?? DEFAULT_FILES.map((f) => ({ ...f })),
  );
  const activeRef = useRef(saved?.active ?? 0);
  const suppressRef = useRef(false);
  const overridesRef = useRef<Record<string, ParamValue>>(
    saved?.overrides ?? {},
  );
  const paramSetsRef = useRef<Record<string, Record<string, ParamValue>>>(
    saved?.paramSets ?? {},
  );
  const paramsJsonRef = useRef("");
  const requestRenderRef = useRef<() => void>(() => {});
  const renderNowRef = useRef<() => void>(() => {}); // immediate render (animation frames bypass the debounce)
  // During frame export: resolved by onResult when the current frame's render lands.
  const frameWaiterRef = useRef<(() => void) | null>(null);
  const exportingRef = useRef(false);
  // Suppress the orbit→re-render loop while we're applying a script-set camera.
  const applyingCameraRef = useRef(false);
  // Save (desktop): baseline of each file's last-saved content, keyed by name,
  // so a tab can show an unsaved-changes dot. Set on open/save; not persisted.
  const savedRef = useRef<Record<string, string>>({});
  const saveActiveRef = useRef<() => void>(() => {});
  const saveAsRef = useRef<() => void>(() => {});
  const menuExportRef = useRef<() => void>(() => {}); // File ▸ Export (latest closure)
  // Desktop auto-update: the hook drives the in-app <UpdateBanner>; the ref lets
  // the one-shot desktop-wiring effect below reach the latest `check` closure
  // (same pattern as saveActiveRef/menuExportRef).
  const updater = useUpdater();
  const checkUpdatesRef = useRef(updater.check);
  checkUpdatesRef.current = updater.check;
  // Latest engine diagnostics (for the main file) — squiggled in the editor when
  // the main tab is active, and badged on the tab otherwise.
  const diagRef = useRef<EngineDiag[]>([]);
  // Animation playback: a share link may carry $t/fps/steps/play-state so the
  // recipient opens on the same frame and speed.
  const sharedAnim = sharedRef.current?.anim;
  const timeRef = useRef(sharedAnim?.t ?? 0); // $t for animation
  const stepRef = useRef(
    Math.round((sharedAnim?.t ?? 0) * (sharedAnim?.steps ?? 20)),
  ); // current animation frame index (0..steps-1)

  const [files, setFiles] = useState<File[]>(filesRef.current);
  const [active, setActive] = useState(activeRef.current);
  const [status, setStatus] = useState<Status>({
    ok: true,
    message: "initializing…",
    triangleCount: 0,
    vertexCount: 0,
    area: 0,
    volume: 0,
    ms: 0,
    echo: "",
    warnings: "",
    error: "",
    geomErrors: "",
    preview: false,
  });
  const [version, setVersion] = useState("");
  // A render is in flight (drives the "rendering…" indicator + Stop button).
  const [rendering, setRendering] = useState(false);
  // Recovery mode: the restored project wasn't auto-rendered because the last
  // render never finished. Shows a banner and waits for the user to press Render.
  const [recovering, setRecovering] = useState(wasStuck);
  // Autosave to localStorage failed (quota exceeded): warn instead of silently
  // dropping the user's work.
  const [saveFailed, setSaveFailed] = useState(false);
  const [consoleOpen, setConsoleOpen] = useState(false);
  const [consoleFilter, setConsoleFilter] = useState<
    "all" | "error" | "warn" | "echo"
  >("all");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  // Monotonic render counter, surfaced as data-render-rev on the status bar so a
  // completed render is observable even though the meta is always visible.
  const [renderRev, setRenderRev] = useState(0);
  // Right-dock layout: spine collapse (null = auto: spine only when no params)
  // and per-section open state. All persisted.
  const [dockPref, setDockPref] = useState<boolean | null>(
    loadPrefs().dockCollapsed,
  );
  const [paramsOpen, setParamsOpen] = useState(loadPrefs().paramsOpen);
  const [modelOpen, setModelOpen] = useState(loadPrefs().modelOpen);
  // Resizable panel sizes (px); null → the default. Persisted on drag release.
  const [editorWidth, setEditorWidth] = useState<number | null>(
    loadPrefs().editorWidth,
  );
  const [dockWidth, setDockWidth] = useState<number | null>(
    loadPrefs().dockWidth,
  );
  const [consoleHeight, setConsoleHeight] = useState<number | null>(
    loadPrefs().consoleHeight,
  );
  const [showGrid, setShowGrid] = useState(loadPrefs().showGrid);
  const [showEdges, setShowEdges] = useState(loadPrefs().showEdges);
  const [showDims, setShowDims] = useState(loadPrefs().showDims);
  const [exportFmt, setExportFmt] = useState<ExportFmt>("stl");
  // Whether the user has manually chosen an export format. Until they do, the
  // format auto-tracks the model: 3MF for multi-color 3D models, STL otherwise.
  const userPickedFmtRef = useRef(false);
  const [is2D, setIs2D] = useState(false);
  const [dims, setDims] = useState<MeshInfo | null>(null);
  const [ortho, setOrtho] = useState(false);
  const [linkHighlight, setLinkHighlight] = useState(linkHighlightRef.current);
  const [fastPreview, setFastPreview] = useState(fastPreviewRef.current);
  const [quality, setQuality] = useState<Quality>(qualityRef.current.quality);
  // Custom-quality $fn (the crash banner tells users to lower it). $fa/$fs get a
  // fuller editor with the Quality popover in a later phase.
  const [customFn, setCustomFn] = useState<number | null>(
    qualityRef.current.customFn,
  );
  const [engineKind, setEngineKind] = useState<EngineKind>(
    engineKindRef.current,
  );
  // OS light/dark appearance. Auto-follows `prefers-color-scheme`; no toggle.
  const [mode, setMode] = useState<ThemeMode>(currentMode);
  const [time, setTime] = useState(sharedAnim?.t ?? 0);
  const [playing, setPlaying] = useState(sharedAnim?.playing ?? false);
  const [fps, setFps] = useState(sharedAnim?.fps ?? 15);
  const [steps, setSteps] = useState(sharedAnim?.steps ?? 20);
  const [schema, setSchema] = useState<Param[]>([]);
  const [overrides, setOverrides] = useState<Record<string, ParamValue>>(
    overridesRef.current,
  );
  const [paramSets, setParamSets] = useState<
    Record<string, Record<string, ParamValue>>
  >(paramSetsRef.current);
  const [shareMsg, setShareMsg] = useState("");
  // Diagnostic counts for the main file (error/warning), for the tab badge.
  const [diagCounts, setDiagCounts] = useState<{
    errors: number;
    warnings: number;
  }>({
    errors: 0,
    warnings: 0,
  });

  function persist() {
    const ok = saveProject({
      files: filesRef.current,
      overrides: overridesRef.current,
      active: activeRef.current,
      paramSets: paramSetsRef.current,
    });
    // Surface a silent data-loss trap: if storage is full, autosave stops and
    // the user would otherwise never know until a reload lost their work.
    setSaveFailed((prev) => (prev === !ok ? prev : !ok));
  }

  useEffect(() => {
    if (!canvasRef.current || !editorHost.current) return;

    const viewer = new Viewer(canvasRef.current, (info) => setDims(info));
    viewerRef.current = viewer;
    // Apply persisted display toggles (defaults are on, so this only bites when
    // the user had turned the grid/edges off).
    const prefs0 = loadPrefs();
    viewer.setGridVisible(prefs0.showGrid);
    viewer.setEdgesVisible(prefs0.showEdges);
    viewer.setDimensionsVisible(prefs0.showDims);

    // Model → code: clicking a face selects the source statement that produced
    // it. Spans index into the main file, so switch to it first if needed.
    const unsubPick = viewer.onPick((span) => {
      if (!linkHighlightRef.current) return;
      // Clicking empty space deselects: dismiss the highlight and leave the
      // editor cursor where it is.
      if (!span) {
        highlightDismissedRef.current = true;
        viewer.highlightSpan(null);
        return;
      }
      const view = viewRef.current;
      if (!view) return;
      if (activeRef.current !== 0) switchTo(0);
      const src = filesRef.current[0].content;
      const from = byteToChar(src, span[0]);
      const to = byteToChar(src, span[1]);
      const v = viewRef.current!;
      v.dispatch({
        selection: { anchor: from, head: to },
        scrollIntoView: true,
      });
      v.focus();
      // Re-enable and highlight the clicked item. An explicit call covers the
      // case where the selection didn't change (re-clicking the same item after a
      // dismiss), which wouldn't fire the selection listener.
      highlightDismissedRef.current = false;
      highlightFromCursorRef.current();
    });

    // Busy transitions drive the Stop/rendering UI and arm crash-recovery. The
    // sentinel is armed only once a render has run past SLOW_RENDER_MS (so a fast
    // render closed mid-flight never trips it); onResult also arms it
    // synchronously before applying the mesh, to catch a freeze while uploading a
    // huge mesh (the worker already returned, so the slow timer wouldn't fire).
    const onBusyChange = (busy: boolean) => {
      setRendering(busy);
      window.clearTimeout(slowTimer.current);
      if (busy) {
        setRecovering(false);
        slowTimer.current = window.setTimeout(
          markRenderPending,
          SLOW_RENDER_MS,
        );
      }
    };
    // Build an engine for the given kind. "openscad" runs OpenSCAD — a locally-
    // installed binary on desktop (falling back to wasm if none is installed), or
    // the vendored wasm build in the browser. "quito" uses the native C++ engine
    // on desktop and the wasm engine in the browser.
    const buildEngine = (
      kind: EngineKind,
    ): Engine | DesktopEngine | DesktopOpenscadEngine => {
      const cb = (r: RenderResponse) => onResult(r);
      if (kind === "openscad") {
        if (!TAURI) return new OpenscadEngine(cb, { onBusyChange });
        const osc = new DesktopOpenscadEngine(cb, { onBusyChange });
        osc.dir = engineDirRef.current; // disk include/use via OPENSCADPATH
        return osc;
      }
      if (!TAURI) return new Engine(cb, { onBusyChange });
      const native = new DesktopEngine(cb, { onBusyChange });
      native.dir = engineDirRef.current; // re-seed disk include/use resolution
      return native;
    };
    engineRef.current = buildEngine(engineKindRef.current);

    // Swap the live engine (toolbar toggle): tear down the old worker, build the
    // new one (which re-seeds the native include/use dir from `engineDirRef`),
    // and re-render.
    swapEngineRef.current = (kind: EngineKind) => {
      engineRef.current?.dispose();
      engineRef.current = buildEngine(kind);
      renderNowRef.current();
    };

    const renderNow = async () => {
      const fs = filesRef.current;
      const ov = overridesRef.current;
      const names = Object.keys(ov);
      const values = names.map((n) => toLiteral(ov[n]));
      // Render-quality overrides ($fn/$fa/$fs), injected like customizer values.
      // A user param of the same name (unusual) wins, so skip any already set.
      for (const [n, v] of Object.entries(
        qualityOverrides(qualityRef.current),
      )) {
        if (!names.includes(n)) {
          names.push(n);
          values.push(v);
        }
      }
      if (timeRef.current !== 0) {
        names.push("$t");
        values.push(String(timeRef.current));
      }
      // Feed the live camera to scripts that read `$vp*` (also lets the engine
      // report back script-set values, which we apply in onResult).
      if (fs[0].content.includes("$vp") && viewerRef.current) {
        const c = viewerRef.current.getCamera();
        names.push("$vpr", "$vpt", "$vpd", "$vpf");
        values.push(
          `[${c.vpr.join(",")}]`,
          `[${c.vpt.join(",")}]`,
          String(c.vpd),
          String(c.vpf),
        );
      }
      const libs = fs.slice(1);
      if (engineRef.current instanceof DesktopEngine) {
        // Native engine resolves include/use from disk (OPENSCADPATH) + the
        // in-memory tabs; no CDN fetch needed. Only the native engine can read
        // disk — the OpenSCAD wasm engine (even on desktop) takes the closure path.
        engineRef.current.render(
          fs[0].content,
          names,
          values,
          libs.map((f) => f.name),
          libs.map((f) => f.content),
          fastPreviewRef.current,
        );
      } else {
        // Wasm engine (Quito or OpenSCAD, in browser or desktop): resolve the
        // include/use closure (fetching libraries), then render with the full
        // file set. Read `engineRef.current` (not a captured local) so a live
        // engine swap takes effect on the next render.
        const { names: fileNames, contents: fileContents } =
          await resolveClosure(fs[0].content, libs, LIB_BASE);
        engineRef.current?.render(
          fs[0].content,
          names,
          values,
          fileNames,
          fileContents,
          fastPreviewRef.current,
        );
      }
    };
    const requestRender = () => {
      window.clearTimeout(debounceTimer.current);
      debounceTimer.current = window.setTimeout(renderNow, 150);
    };
    requestRenderRef.current = requestRender;
    renderNowRef.current = () => {
      renderNow();
    };

    // When the model reads `$vp*`, re-render (debounced) as the camera moves so
    // the geometry tracks the viewport. Suppressed while applying a script-set
    // camera to avoid a feedback loop.
    const unsubCamera = viewer.onCameraChange(() => {
      if (applyingCameraRef.current || exportingRef.current) return;
      if (filesRef.current[0].content.includes("$vp"))
        requestRenderRef.current();
    });

    const view = new EditorView({
      state: EditorState.create({
        doc: filesRef.current[activeRef.current].content,
        extensions: [
          // ⌘↵ renders. Highest precedence so it beats basicSetup's
          // defaultKeymap, where Mod-Enter is insertBlankLine.
          Prec.highest(
            keymap.of([
              {
                key: "Mod-Enter",
                preventDefault: true,
                run: () => {
                  renderNowRef.current();
                  return true;
                },
              },
            ]),
          ),
          basicSetup,
          keymap.of([
            // ⌘S / ⌘⇧S save the active tab to disk (desktop). preventDefault
            // stops the browser's own save dialog even in the web build.
            {
              key: "Mod-s",
              preventDefault: true,
              run: () => {
                saveActiveRef.current();
                return true;
              },
            },
            {
              key: "Mod-Shift-s",
              preventDefault: true,
              run: () => {
                saveAsRef.current();
                return true;
              },
            },
            indentWithTab,
          ]),
          openscad(),
          // After basicSetup so our HighlightStyle beats the fallback default.
          // Reconfigured live by the [mode] effect below.
          themeComp.current.of(themeExts(currentMode())),
          EditorView.updateListener.of((u) => {
            if (u.docChanged && !suppressRef.current) {
              const idx = activeRef.current;
              const next = filesRef.current.slice();
              next[idx] = { ...next[idx], content: u.state.doc.toString() };
              filesRef.current = next;
              setFiles(next);
              persist();
              requestRender();
            }
            // Code → model: highlight the geometry under the cursor as it moves.
            // A genuine cursor move re-enables highlighting after a dismiss.
            if (u.selectionSet) highlightDismissedRef.current = false;
            if (u.selectionSet || u.docChanged) {
              highlightFromCursorRef.current();
            }
          }),
        ],
      }),
      parent: editorHost.current,
    });
    viewRef.current = view;

    if (wasStuck) {
      // The last render never finished (froze/crashed the tab). Stay idle instead
      // of re-triggering it; the recovery banner lets the user simplify the script
      // or render on demand. The sentinel is deliberately left ARMED: safe mode
      // must survive repeated relaunches (a user who just quits from here would
      // otherwise auto-render the same too-heavy model next launch). It clears
      // only when a render genuinely completes — an edit/Render-anyway that
      // finishes, a New project, or a loaded example.
      setStatus((s) => ({
        ...s,
        ok: false,
        message: "render paused — the last render didn't finish",
      }));
    } else {
      renderNow(); // initial render
    }

    // A project opened from a share link isn't in localStorage yet — persist it
    // now so a plain reload (or losing the hash) keeps the shared work.
    if (sharedRef.current) persist();

    // Desktop wiring: external-edit reload, native menu, and open-with.
    const unlisteners: (() => void)[] = [];
    if (TAURI) {
      // Seed saved baselines for any restored files that already have a disk path,
      // and (re)arm watchers for them so external edits reload after a relaunch.
      const paths = filesRef.current
        .map((f) => f.path)
        .filter((p): p is string => !!p);
      for (const f of filesRef.current)
        if (f.path) savedRef.current[f.name] = f.content;
      if (paths.length) void watchFiles(paths);

      // Live-reload a file edited in an external editor. Route by path to the
      // right tab; self-saves are already suppressed on the Rust side.
      onFileChanged(({ path, content }) => applyExternalEdit(path, content))
        .then((u) => unlisteners.push(u))
        .catch(() => {});

      // Native menu items relay their action id here.
      onMenuAction((action) => {
        switch (action) {
          case "new":
            newProject();
            break;
          case "open":
            void openNative();
            break;
          case "save":
            saveActiveRef.current();
            break;
          case "save-as":
            saveAsRef.current();
            break;
          case "export":
            menuExportRef.current();
            break;
          case "reset-view":
            viewerRef.current?.resetView();
            break;
          case "check-updates":
            void checkUpdatesRef.current(true);
            break;
        }
      })
        .then((u) => unlisteners.push(u))
        .catch(() => {});

      // Open-with: a warm event, plus a path buffered from a cold launch.
      const openByPath = async (p: string) => {
        try {
          const f = await openScadPath(p);
          setMainFile(f.name, f.content, f.dir, f.path);
          void watchFiles([f.path]);
        } catch {
          /* unreadable / unavailable */
        }
      };
      onOpenPath((p) => void openByPath(p))
        .then((u) => unlisteners.push(u))
        .catch(() => {});
      takePendingOpen()
        .then((p) => {
          if (p) void openByPath(p);
        })
        .catch(() => {});

      // Silent update check on launch: shows the banner only if an update is
      // available, stays quiet on "up to date" and on errors (offline, etc.).
      void checkUpdatesRef.current(false);
    }

    // App-level keyboard shortcuts (web actions). ⌘↵ inside the editor is caught
    // by the high-precedence CM keymap above, so skip it here when the editor is
    // focused to avoid rendering twice.
    const onKeyDown = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      const key = e.key.toLowerCase();
      if (mod && key === "k") {
        e.preventDefault();
        setPaletteOpen((o) => !o);
        return;
      }
      if (mod && key === "j") {
        e.preventDefault();
        setConsoleOpen((o) => !o);
        return;
      }
      if (mod && e.shiftKey && key === "f") {
        e.preventDefault();
        viewerRef.current?.fit();
        return;
      }
      if (mod && key === "enter") {
        const inEditor = (e.target as HTMLElement)?.closest?.(".cm-editor");
        if (!inEditor) {
          e.preventDefault();
          renderNowRef.current();
        }
        return;
      }
      // Escape deselects the highlighted item (like clicking empty preview).
      if (e.key === "Escape" && linkHighlightRef.current) {
        highlightDismissedRef.current = true;
        viewerRef.current?.highlightSpan(null);
      }
    };
    window.addEventListener("keydown", onKeyDown);

    return () => {
      view.destroy();
      unsubCamera();
      unsubPick();
      viewer.dispose();
      window.clearTimeout(slowTimer.current);
      window.removeEventListener("keydown", onKeyDown);
      for (const u of unlisteners) u();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Follow the OS appearance: subscribe once to prefers-color-scheme and mirror
  // changes into `mode`. The [mode] effect below propagates them everywhere.
  useEffect(() => {
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setMode(currentMode());
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  // Apply the current appearance to the document, the editor (via the theme
  // Compartment), and the 3D viewer. Runs on mount and every flip. Declared
  // after the mount effect so viewRef/viewerRef are already set on the first
  // flip; the no-op guard covers the (unlikely) pre-mount case.
  useEffect(() => {
    document.documentElement.dataset.theme = mode;
    document.documentElement.style.colorScheme = mode;
    viewerRef.current?.setTheme(mode);
    if (viewRef.current) {
      viewRef.current.dispatch({
        effects: themeComp.current.reconfigure(themeExts(mode)),
      });
    }
  }, [mode]);

  // Animation driver: while playing, advance $t one frame every 1000/fps ms,
  // wrapping after `steps` frames ($t = frame/steps, matching OpenSCAD). Frames
  // render immediately (bypassing the edit debounce); the engine's worker-
  // terminate cancellation drops any frame still rendering when the next fires,
  // so a slow model just lowers the effective frame rate instead of piling up.
  useEffect(() => {
    if (!playing) return;
    const n = Math.max(1, Math.round(steps));
    const period = 1000 / Math.max(1, fps);
    const id = window.setInterval(() => {
      stepRef.current = (stepRef.current + 1) % n;
      const t = stepRef.current / n;
      timeRef.current = t;
      setTime(t);
      renderNowRef.current();
    }, period);
    return () => window.clearInterval(id);
  }, [playing, fps, steps]);

  /** Jump to an absolute $t (0–1), syncing the frame index so playback resumes
   *  from here. Used by the scrub slider. */
  function seekTime(t: number) {
    timeRef.current = t;
    setTime(t);
    stepRef.current = Math.round(t * Math.max(1, Math.round(steps)));
    renderNowRef.current();
  }

  /** Replace the rendered (first) file's content — from a native open or an
   *  external-edit reload — updating the editor if that tab is active. When a
   *  disk `path` is given the tab remembers it (so ⌘S writes there) and the
   *  content becomes the new saved baseline (no unsaved-changes dot). */
  function setMainFile(
    name: string,
    content: string,
    dir?: string,
    path?: string,
  ) {
    const next = filesRef.current.slice();
    next[0] = { name, content, path: path ?? next[0].path };
    filesRef.current = next;
    setFiles(next);
    if (path) savedRef.current[name] = content;
    if (dir) {
      engineDirRef.current = dir;
      const e = engineRef.current;
      if (e instanceof DesktopEngine || e instanceof DesktopOpenscadEngine)
        e.dir = dir;
    }
    if (activeRef.current === 0 && viewRef.current) {
      const view = viewRef.current;
      suppressRef.current = true;
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: content },
      });
      suppressRef.current = false;
    }
    persist();
    requestRenderRef.current();
  }

  /** Apply an external-editor change to whichever tab owns `path` (self-saves
   *  are already filtered out on the Rust side). Unknown paths fall back to the
   *  main tab, preserving the pre-multi-file behavior. */
  function applyExternalEdit(path: string, content: string) {
    const idx = filesRef.current.findIndex((f) => f.path === path);
    if (idx <= 0) {
      setMainFile(
        filesRef.current[0].name,
        content,
        undefined,
        filesRef.current[0].path,
      );
      return;
    }
    const next = filesRef.current.slice();
    next[idx] = { ...next[idx], content };
    filesRef.current = next;
    setFiles(next);
    savedRef.current[next[idx].name] = content; // disk is the new baseline
    if (activeRef.current === idx && viewRef.current) {
      const view = viewRef.current;
      suppressRef.current = true;
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: content },
      });
      suppressRef.current = false;
    }
    persist();
    requestRenderRef.current();
  }

  async function openNative() {
    try {
      const f = await openScadFile();
      if (f) {
        setMainFile(f.name, f.content, f.dir, f.path);
        void watchFiles([f.path]);
      }
    } catch {
      /* dialog cancelled / unavailable */
    }
  }

  /** Persist a disk path onto a tab and update its saved baseline + editor name. */
  function recordSaved(idx: number, path: string, content: string) {
    const next = filesRef.current.slice();
    const name = basename(path);
    next[idx] = { ...next[idx], name, content, path };
    filesRef.current = next;
    setFiles(next);
    savedRef.current[name] = content;
    persist();
  }

  /** Save the active tab to disk (⌘S / File ▸ Save). Prompts (Save As) when the
   *  tab has no disk path yet. Desktop only — the browser autosaves to storage. */
  async function saveActive(forceDialog = false) {
    if (!TAURI) return;
    const idx = activeRef.current;
    const f = filesRef.current[idx];
    const content = viewRef.current?.state.doc.toString() ?? f.content;
    try {
      if (f.path && !forceDialog) {
        await saveSource(f.path, content);
        recordSaved(idx, f.path, content);
      } else {
        const path = await saveSourceAs(content, f.name);
        if (!path) return; // cancelled
        recordSaved(idx, path, content);
        // Main file's directory drives include/use resolution on either native
        // engine (Quito's disk resolver / OpenSCAD's OPENSCADPATH).
        if (idx === 0) {
          const d = path.slice(0, path.length - basename(path).length) || ".";
          engineDirRef.current = d;
          const e = engineRef.current;
          if (e instanceof DesktopEngine || e instanceof DesktopOpenscadEngine)
            e.dir = d;
        }
        void watchFiles(
          filesRef.current.map((x) => x.path).filter((p): p is string => !!p),
        );
      }
    } catch (e) {
      setStatus((s) => ({
        ...s,
        ok: false,
        error: `save failed: ${String(e)}`,
        message: "save failed",
      }));
      setConsoleOpen(true);
    }
  }

  /** Push the current engine diagnostics into the editor — but only when the
   *  main file (index 0) is showing, since spans index into the main source. On
   *  any other tab, clear the squiggles (the main tab shows a badge instead). */
  function applyDiagnostics() {
    const view = viewRef.current;
    if (!view) return;
    const diags =
      activeRef.current === 0
        ? toCmDiagnostics(diagRef.current, filesRef.current[0].content)
        : [];
    view.dispatch(setDiagnostics(view.state, diags));
  }

  /** Code → model: highlight the geometry produced by the statement under the
   *  editor cursor. Only the main file participates (provenance spans index into
   *  the main source); on any other tab the highlight is cleared. */
  function highlightFromCursor() {
    const viewer = viewerRef.current;
    const view = viewRef.current;
    if (!viewer || !view) return;
    if (
      !linkHighlightRef.current ||
      activeRef.current !== 0 ||
      highlightDismissedRef.current
    ) {
      viewer.highlightSpan(null);
      return;
    }
    // Use the selection's start (not its head): a model→code click selects the
    // whole clicked statement `[from,to)`, and the head lands on the *exclusive*
    // end `to`, which no half-open span contains — so a click would resolve to a
    // parent or nothing. The start byte sits inside the clicked statement, so the
    // click lights exactly that item, matching the code→model direction.
    const pos = view.state.selection.main.from;
    const byte = charToByte(filesRef.current[0].content, pos);
    // Among every span (at any nesting level) that contains that byte, pick the
    // narrowest — the tightest enclosing statement. highlightSpan then lights all
    // geometry whose stack contains it (that statement's whole subtree).
    let best: Span | null = null;
    for (const g of provenanceRef.current) {
      for (const s of g.spans) {
        if (
          byte >= s[0] &&
          byte < s[1] &&
          (!best || s[1] - s[0] < best[1] - best[0])
        ) {
          best = s;
        }
      }
    }
    viewer.highlightSpan(best);
  }

  /** Toggle editor↔preview highlighting (both directions) and remember the
   *  choice. Turning it off clears any live overlay; turning it on re-applies
   *  the highlight for the current cursor. */
  function toggleLinkHighlight() {
    const next = !linkHighlightRef.current;
    linkHighlightRef.current = next;
    setLinkHighlight(next);
    savePrefs({ linkHighlight: next });
    if (next) highlightFromCursor();
    else viewerRef.current?.highlightSpan(null);
  }

  /** Toggle the fast (non-watertight) preview and re-render so the change is
   *  visible immediately. Remembered across sessions. */
  function toggleFastPreview() {
    const next = !fastPreviewRef.current;
    fastPreviewRef.current = next;
    setFastPreview(next);
    savePrefs({ fastPreview: next });
    renderNowRef.current?.();
  }

  /** Change the render-quality preset (or the custom $fn), persist it, and
   *  re-render so the new resolution shows immediately. */
  function setQualityPref(next: Partial<QualitySettings>) {
    qualityRef.current = { ...qualityRef.current, ...next };
    if (next.quality !== undefined) setQuality(next.quality);
    if (next.customFn !== undefined) setCustomFn(next.customFn);
    savePrefs(next);
    renderNowRef.current?.();
  }

  // Effective dock collapse: explicit pref wins, else auto-spine when no params.
  const dockCollapsed = dockPref ?? schema.length === 0;
  function toggleDock() {
    const v = !dockCollapsed;
    setDockPref(v);
    savePrefs({ dockCollapsed: v });
  }
  function toggleParamsSection() {
    const v = !paramsOpen;
    setParamsOpen(v);
    savePrefs({ paramsOpen: v });
  }
  function toggleModelSection() {
    const v = !modelOpen;
    setModelOpen(v);
    savePrefs({ modelOpen: v });
  }

  // --- resizable panels: apply a pointer delta, then persist on release ---
  const effEditorW = editorWidth ?? EDITOR_W_DEFAULT;
  const effDockW = dockWidth ?? DOCK_W_DEFAULT;
  const effConsoleH = consoleHeight ?? CONSOLE_H_DEFAULT;
  // Mirror the live sizes so the drag's pointerup closure (bound at pointerdown)
  // persists the *current* values, not the ones captured at drag start.
  const sizeRef = useRef({ editorWidth, dockWidth, consoleHeight });
  sizeRef.current = { editorWidth, dockWidth, consoleHeight };
  // Deltas arrive incrementally per pointermove, so accumulate with functional
  // updates rather than adding to a value captured at drag start.
  function dragEditor(delta: number) {
    setEditorWidth((w) =>
      clampNum((w ?? EDITOR_W_DEFAULT) + delta, 260, window.innerWidth - 480),
    );
  }
  function dragDock(delta: number) {
    // The handle is left of the dock, so dragging right shrinks the dock.
    setDockWidth((w) =>
      clampNum((w ?? DOCK_W_DEFAULT) - delta, 200, window.innerWidth - 480),
    );
  }
  function dragConsole(delta: number) {
    // The handle is atop the console, so dragging up grows it.
    setConsoleHeight((h) =>
      clampNum((h ?? CONSOLE_H_DEFAULT) - delta, 80, window.innerHeight - 220),
    );
  }
  const persistSizes = () => savePrefs(sizeRef.current);

  function toggleGrid(v: boolean) {
    setShowGrid(v);
    viewerRef.current?.setGridVisible(v);
    savePrefs({ showGrid: v });
  }
  function toggleEdges(v: boolean) {
    setShowEdges(v);
    viewerRef.current?.setEdgesVisible(v);
    savePrefs({ showEdges: v });
  }
  function toggleDims(v: boolean) {
    setShowDims(v);
    viewerRef.current?.setDimensionsVisible(v);
    savePrefs({ showDims: v });
  }
  function setOrthoProjection(next: boolean) {
    viewerRef.current?.setProjection(next ? "orthographic" : "perspective");
    setOrtho(next);
  }

  /** Swap the render engine between Quito and the vendored OpenSCAD wasm, then
   *  re-render on the new engine. Remembered across sessions. On desktop, "quito"
   *  is the native engine and "openscad" runs the OpenSCAD wasm in-webview. */
  function toggleEngine() {
    const next: EngineKind =
      engineKindRef.current === "openscad" ? "quito" : "openscad";
    engineKindRef.current = next;
    setEngineKind(next);
    savePrefs({ engine: next });
    swapEngineRef.current(next);
  }

  function switchTo(idx: number) {
    if (idx === activeRef.current || !viewRef.current) return;
    activeRef.current = idx;
    setActive(idx);
    const view = viewRef.current;
    suppressRef.current = true;
    view.dispatch({
      changes: {
        from: 0,
        to: view.state.doc.length,
        insert: filesRef.current[idx].content,
      },
    });
    suppressRef.current = false;
    applyDiagnostics();
    view.focus();
    persist();
  }

  async function onShare() {
    // Only attach animation state when it differs from the defaults, so a
    // still, unedited project produces the same compact link as before.
    const animAtDefault = time === 0 && !playing && fps === 15 && steps === 20;
    const url = shareUrl(
      {
        files: filesRef.current,
        overrides: overridesRef.current,
        active: activeRef.current,
      },
      animAtDefault ? undefined : { t: time, fps, steps, playing },
    );
    // Reflect the link in the address bar (replaceState avoids a scroll/nav).
    try {
      window.history.replaceState(null, "", url);
    } catch {
      /* ignore */
    }
    try {
      await navigator.clipboard.writeText(url);
      setShareMsg("Link copied!");
    } catch {
      setShareMsg("Link in address bar");
    }
    window.setTimeout(() => setShareMsg(""), 2000);
  }

  /** Download the active file's source as a .scad file (browser only; desktop
   *  has native Save/Save As). */
  function onDownloadScad() {
    const file = filesRef.current[activeRef.current];
    if (!file) return;
    const name = file.name.endsWith(".scad") ? file.name : `${file.name}.scad`;
    downloadBlob(new TextEncoder().encode(file.content), name);
  }

  function newProject() {
    if (!window.confirm("Discard the current project and start fresh?")) return;
    clearProject();
    // Drop any share-link hash so a reload doesn't restore the shared project.
    sharedRef.current = null;
    try {
      window.history.replaceState(
        null,
        "",
        window.location.pathname + window.location.search,
      );
    } catch {
      /* ignore */
    }
    const fresh = DEFAULT_FILES.map((f) => ({ ...f }));
    filesRef.current = fresh;
    overridesRef.current = {};
    setFiles(fresh);
    setOverrides({});
    activeRef.current = -1;
    switchTo(0);
    requestRenderRef.current();
  }

  /** Replace the whole project with a curated example (from the Examples menu). */
  function loadExample(idx: number) {
    const ex = EXAMPLES[idx];
    if (!ex) return;
    if (
      !window.confirm(
        `Load the "${ex.label}" example? This replaces the current project.`,
      )
    )
      return;
    sharedRef.current = null;
    try {
      window.history.replaceState(
        null,
        "",
        window.location.pathname + window.location.search,
      );
    } catch {
      /* ignore */
    }
    const fresh = ex.files.map((f) => ({ ...f }));
    filesRef.current = fresh;
    overridesRef.current = {};
    setFiles(fresh);
    setOverrides({});
    activeRef.current = -1;
    switchTo(0);
    persist();
    requestRenderRef.current();
  }

  function addFile() {
    const fs = filesRef.current;
    let n = fs.length;
    let name = `lib${n}.scad`;
    while (fs.some((f) => f.name === name)) name = `lib${++n}.scad`;
    const next = [...fs, { name, content: `// ${name}\n` }];
    filesRef.current = next;
    setFiles(next);
    switchTo(next.length - 1);
    persist();
  }

  function deleteFile(idx: number) {
    if (idx === 0) return; // main is not deletable
    const next = filesRef.current.filter((_, i) => i !== idx);
    filesRef.current = next;
    setFiles(next);
    // Pick a new active file, then force the editor to swap to it.
    let na = activeRef.current;
    if (na === idx) na = idx - 1;
    else if (na > idx) na -= 1;
    activeRef.current = -1;
    switchTo(na);
    persist();
    requestRenderRef.current();
  }

  function renameFile(idx: number) {
    if (idx === 0) return; // keep main.scad stable
    const cur = filesRef.current[idx].name;
    const name = window.prompt("Rename file", cur);
    if (!name || name === cur || filesRef.current.some((f) => f.name === name))
      return;
    const next = filesRef.current.slice();
    next[idx] = { ...next[idx], name };
    filesRef.current = next;
    setFiles(next);
    persist();
    requestRenderRef.current();
  }

  function onResult(r: RenderResponse) {
    // Arm crash-recovery before touching the mesh: applying a very large mesh can
    // freeze the main thread here (the worker already returned), and a set-then-
    // cleared sentinel across this synchronous block persists only if we never
    // reach the clear below — i.e. exactly when the tab froze/crashed applying it.
    // A *stopped* result (watchdog timeout / user Stop) is exempt: the render
    // never finished, so the sentinel must stay in whatever state the slow-timer
    // left it (armed iff the render ran past SLOW_RENDER_MS) — clearing/re-arming
    // it here would either disarm recovery for a too-heavy render or falsely arm
    // it for a quick Stop.
    window.clearTimeout(slowTimer.current);
    if (!r.stopped) markRenderPending();

    if (r.version) setVersion(r.version);

    // Unblock a frame-export step waiting on this render (mesh is applied below).
    const frameWaiter = frameWaiterRef.current;

    // Inline diagnostics: parse the structured channel, remember it (for the
    // tab badge), and squiggle it in the editor when the main tab is showing.
    let diags: EngineDiag[] = [];
    try {
      diags = JSON.parse(r.diagnostics || "[]") as EngineDiag[];
    } catch {
      diags = [];
    }
    diagRef.current = diags;
    setDiagCounts({
      errors: diags.filter((d) => d.severity === "error").length,
      warnings: diags.filter((d) => d.severity === "warning").length,
    });
    applyDiagnostics();

    if (r.params && r.params !== paramsJsonRef.current) {
      paramsJsonRef.current = r.params;
      const next = parseSchema(r.params);
      setSchema(next);
      const kept: Record<string, ParamValue> = {};
      for (const [k, v] of Object.entries(overridesRef.current)) {
        if (next.some((p) => p.name === k)) kept[k] = v;
      }
      if (
        Object.keys(kept).length !== Object.keys(overridesRef.current).length
      ) {
        overridesRef.current = kept;
        setOverrides(kept);
      }
    }

    if (r.ok) {
      lastPositions.current = r.positions;
      // Colored preview groups (present only when the model uses color/`#`/`%`).
      let groups: PreviewGroup[] = [];
      if (r.groups) {
        try {
          groups = JSON.parse(r.groups) as PreviewGroup[];
        } catch {
          groups = [];
        }
      }
      lastPreview.current = { positions: r.previewPositions, groups };
      if (groups.length > 0) {
        viewerRef.current?.setColoredMesh(
          r.previewPositions,
          r.previewNormals,
          groups,
        );
      } else {
        viewerRef.current?.setMesh(r.positions, r.normals);
      }
      // Provenance channel for editor↔preview linking (picking + highlight).
      let prov: ProvenanceGroup[] = [];
      if (r.provenance) {
        try {
          prov = JSON.parse(r.provenance) as ProvenanceGroup[];
        } catch {
          prov = [];
        }
      }
      provenanceRef.current = prov;
      viewerRef.current?.setProvenance(
        r.provenancePositions,
        r.provenanceNormals,
        prov,
      );
      // Re-apply the code→model highlight for the current cursor (setProvenance
      // cleared the stale overlay).
      highlightFromCursor();
      // A script that assigned `$vp*` drives the camera: apply it when the
      // returned viewport differs from the camera we sent.
      if (r.viewport && viewerRef.current && !exportingRef.current) {
        applyScriptCamera(r.viewport);
      }
      // Offer vector formats for 2D models, mesh formats for 3D; keep the
      // selected format valid when the model's dimensionality changes.
      setIs2D(r.is2D);
      const multiColor = distinctExportColors(groups) > 1;
      setExportFmt((f) => {
        if (r.is2D) return FORMATS_2D.includes(f) ? f : "dxf";
        // Until the user picks a format, default multi-color models to 3MF (which
        // preserves the colors) and everything else to STL.
        if (!userPickedFmtRef.current) return multiColor ? "3mf" : "stl";
        return FORMATS_3D.includes(f) ? f : "stl";
      });
      setStatus({
        ok: true,
        message: r.geomErrors
          ? `${r.triangleCount.toLocaleString()} triangles · geometry errors`
          : `${r.triangleCount.toLocaleString()} triangles`,
        triangleCount: r.triangleCount,
        vertexCount: r.vertexCount,
        area: r.area,
        volume: r.volume,
        ms: r.ms,
        echo: r.echo,
        warnings: r.warnings,
        error: "",
        geomErrors: r.geomErrors,
        preview: r.preview ?? false,
      });
      // A degraded render still shows a mesh, but the user should know it's
      // wrong somewhere — pop the console so the error is visible.
      if (r.geomErrors) setConsoleOpen(true);
    } else {
      setStatus((s) => ({
        ...s,
        ok: false,
        message: r.error,
        ms: r.ms,
        echo: r.echo,
        warnings: r.warnings,
        error: r.error,
        geomErrors: r.geomErrors,
      }));
      setConsoleOpen(true);
    }

    if (frameWaiter) {
      frameWaiterRef.current = null;
      frameWaiter();
    }

    // Settle the crash-recovery sentinel. A genuine render (and its mesh
    // application) completed without freezing the tab, so it's disarmed. Last
    // statement on purpose: if applying a huge mesh above hangs the main thread,
    // we never reach here and the sentinel stays set so the next load recovers.
    // A *stopped* result (watchdog/Stop) is exempt — see settleRenderPending.
    settleRenderPending(!!r.stopped);

    // Bump a monotonic revision so a "new render landed" is observable (the
    // status meta is now always visible, so its presence no longer signals it).
    setRenderRev((n) => n + 1);
  }

  // A console line carries a source span only when the structured diagnostics
  // array has a matching message with a real (byte ≥ 0) offset. Echo output and
  // geom-error prose never resolve to a span, so they stay non-clickable — making
  // every line *look* clickable is worse than making only the real ones clickable.
  type ConsoleLine = {
    kind: "error" | "warn" | "echo";
    text: string;
    span?: Span;
  };
  const spanFor = (
    severity: "error" | "warning",
    message: string,
  ): Span | undefined => {
    const d = diagRef.current.find(
      (x) => x.severity === severity && x.message === message && x.start >= 0,
    );
    return d ? [d.start, d.end] : undefined;
  };
  const consoleLines: ConsoleLine[] = [];
  if (status.error)
    consoleLines.push({
      kind: "error",
      text: status.error,
      span: spanFor("error", status.error),
    });
  // Recoverable geometry errors: shown red like a hard error, but the model is
  // still rendered (degraded) alongside them. Prose, so never clickable.
  for (const e of status.geomErrors.split("\n").filter(Boolean))
    consoleLines.push({ kind: "error", text: `GEOMETRY ERROR: ${e}` });
  for (const w of status.warnings.split("\n").filter(Boolean))
    consoleLines.push({
      kind: "warn",
      text: `WARNING: ${w}`,
      span: spanFor("warning", w),
    });
  for (const e of status.echo.split("\n").filter(Boolean))
    consoleLines.push({ kind: "echo", text: e });

  /** Jump the editor cursor to a diagnostic's source span (main file). Mirrors
   *  the model→code pick path: switch to the main tab, map bytes→chars, select. */
  function jumpToSpan(span: Span) {
    const view = viewRef.current;
    if (!view) return;
    if (activeRef.current !== 0) switchTo(0);
    const src = filesRef.current[0].content;
    const from = byteToChar(src, span[0]);
    const to = byteToChar(src, span[1]);
    view.dispatch({
      selection: { anchor: from, head: to },
      scrollIntoView: true,
    });
    view.focus();
  }

  function setOverride(name: string, value: ParamValue) {
    const next = { ...overridesRef.current, [name]: value };
    overridesRef.current = next;
    setOverrides(next);
    persist();
    requestRenderRef.current();
  }

  function resetOverrides() {
    overridesRef.current = {};
    setOverrides({});
    persist();
    requestRenderRef.current();
  }

  // ---- customizer parameter sets (presets) ----
  function commitParamSets(next: Record<string, Record<string, ParamValue>>) {
    paramSetsRef.current = next;
    setParamSets(next);
    persist();
  }

  /** Apply a saved set: its values become the current overrides (only params in
   *  the active schema survive). */
  function applyPreset(name: string) {
    const set = paramSetsRef.current[name];
    if (!set) return;
    const next: Record<string, ParamValue> = {};
    for (const p of schema) if (p.name in set) next[p.name] = set[p.name];
    overridesRef.current = next;
    setOverrides(next);
    persist();
    requestRenderRef.current();
  }

  /** Snapshot the current effective values (overrides + untouched defaults) as a
   *  named set. */
  function savePreset() {
    const name = window.prompt("Save parameter set as:");
    if (!name) return;
    const snapshot: Record<string, ParamValue> = {};
    for (const p of schema)
      snapshot[p.name] = overridesRef.current[p.name] ?? p.value;
    commitParamSets({ ...paramSetsRef.current, [name]: snapshot });
  }

  function deletePreset(name: string) {
    const next = { ...paramSetsRef.current };
    delete next[name];
    commitParamSets(next);
  }

  function exportPresets() {
    const json = toParamSetsJson(paramSetsRef.current);
    downloadBlob(new TextEncoder().encode(json), "params.json");
  }

  async function importPresets(file: globalThis.File) {
    try {
      const text = await file.text();
      const sets = fromParamSetsJson(text, schema);
      commitParamSets({ ...paramSetsRef.current, ...sets });
    } catch (e) {
      setStatus((s) => ({ ...s, error: `import failed: ${String(e)}` }));
      setConsoleOpen(true);
    }
  }

  /** Apply a script-assigned camera (`$vp*` from the render result) to the
   *  viewer, but only where it differs from the camera we sent. */
  function applyScriptCamera(json: string) {
    const viewer = viewerRef.current;
    if (!viewer) return;
    let vp: {
      vpr?: [number, number, number] | null;
      vpt?: [number, number, number] | null;
      vpd?: number | null;
      vpf?: number | null;
    };
    try {
      vp = JSON.parse(json);
    } catch {
      return;
    }
    const cur = viewer.getCamera();
    const nearN = (a: number | null | undefined, b: number) =>
      a == null || Math.abs(a - b) < 1e-3;
    const nearV = (
      a: [number, number, number] | null | undefined,
      b: [number, number, number],
    ) => !a || a.every((x, i) => Math.abs(x - b[i]) < 1e-3);
    const changed =
      !nearV(vp.vpr, cur.vpr) ||
      !nearV(vp.vpt, cur.vpt) ||
      !nearN(vp.vpd, cur.vpd) ||
      !nearN(vp.vpf, cur.vpf);
    if (!changed) return;
    applyingCameraRef.current = true;
    viewer.setCamera(vp);
    requestAnimationFrame(() => {
      applyingCameraRef.current = false;
    });
  }

  /** Capture the viewer as a PNG — native save dialog on desktop, download in
   *  the browser. */
  async function onSavePng() {
    const viewer = viewerRef.current;
    if (!viewer) return;
    try {
      const blob = await viewer.capturePng();
      if (TAURI) {
        await saveImageNative(new Uint8Array(await blob.arrayBuffer()));
      } else {
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = "quito.png";
        a.click();
        URL.revokeObjectURL(url);
      }
    } catch (e) {
      setStatus((s) => ({ ...s, error: `PNG export failed: ${String(e)}` }));
      setConsoleOpen(true);
    }
  }

  /** Render `steps` animation frames ($t = i/steps) and download a zip of PNGs.
   *  Each frame is rendered and awaited before capture. */
  async function onExportFrames() {
    const viewer = viewerRef.current;
    if (!viewer || exportingRef.current) return;
    exportingRef.current = true;
    setPlaying(false);
    const n = Math.max(1, Math.round(steps));
    const savedT = timeRef.current;
    const savedStep = stepRef.current;
    const pad = Math.max(5, String(n - 1).length);
    const frames: { name: string; data: Uint8Array }[] = [];
    try {
      for (let i = 0; i < n; i++) {
        timeRef.current = i / n;
        setTime(i / n);
        await new Promise<void>((resolve) => {
          frameWaiterRef.current = resolve;
          renderNowRef.current();
        });
        await new Promise((r) => requestAnimationFrame(() => r(null)));
        const blob = await viewer.capturePng();
        frames.push({
          name: `frame${String(i).padStart(pad, "0")}.png`,
          data: new Uint8Array(await blob.arrayBuffer()),
        });
      }
      downloadBlob(zipFiles(frames), "frames.zip");
    } catch (e) {
      setStatus((s) => ({ ...s, error: `frame export failed: ${String(e)}` }));
      setConsoleOpen(true);
    } finally {
      frameWaiterRef.current = null;
      exportingRef.current = false;
      timeRef.current = savedT;
      stepRef.current = savedStep;
      setTime(savedT);
      renderNowRef.current();
    }
  }

  async function onDownload(format: ExportFmt) {
    if (status.triangleCount === 0) return;
    const fs = filesRef.current;
    const ov = overridesRef.current;
    const names = Object.keys(ov);
    const values = names.map((n) => toLiteral(ov[n]));
    const libs = fs.slice(1);

    // On desktop, write bytes we build client-side via a native save dialog;
    // in the browser, trigger an anchor download. Used by the wasm-engine export
    // paths below (the native engine has its own `save_model` re-render path).
    const saveExport = (data: Uint8Array, ext: string) =>
      TAURI
        ? void saveBytesNative(data, ext)
        : downloadBlob(data, `quito.${ext}`);

    // The native re-render export applies only when the native engine produced
    // what's on screen. With the OpenSCAD wasm engine active (even on desktop),
    // fall through to the client-side build paths so the file matches the view.
    if (engineRef.current instanceof DesktopEngine) {
      // Native: re-render on the native engine and write via a save dialog, so
      // the exported model is welded/exact (not derived from the render soup).
      void saveModelNative(
        format,
        fs[0].content,
        names,
        values,
        libs.map((f) => f.name),
        libs.map((f) => f.content),
      );
      return;
    }

    // 2D vector formats need the exact contours, so re-render in a worker.
    if (format === "dxf" || format === "svg") {
      try {
        const { names: fileNames, contents: fileContents } =
          await resolveClosure(fs[0].content, libs, LIB_BASE);
        const text = await export2dBrowser({
          source: fs[0].content,
          names,
          values,
          fileNames,
          fileContents,
          format,
        });
        saveExport(new TextEncoder().encode(text), format);
      } catch (err) {
        setStatus((s) => ({ ...s, error: `export failed: ${String(err)}` }));
        setConsoleOpen(true);
      }
      return;
    }

    // 3D mesh formats: build client-side from the last render soup. But that soup
    // may be a fast, non-watertight preview — never export that. Re-render exact
    // in a throwaway worker so the file is watertight regardless of the toggle.
    let pos = lastPositions.current;
    if (pos.length === 0) return;
    if (status.preview) {
      try {
        const { names: fileNames, contents: fileContents } =
          await resolveClosure(fs[0].content, libs, LIB_BASE);
        pos = await renderMeshExactBrowser({
          source: fs[0].content,
          names,
          values,
          fileNames,
          fileContents,
        });
      } catch (err) {
        setStatus((s) => ({ ...s, error: `export failed: ${String(err)}` }));
        setConsoleOpen(true);
        return;
      }
    }
    // Colored 3MF: one object per non-`%` color group (falls back to fused 3MF).
    if (format === "3mf") {
      const { positions, groups } = lastPreview.current;
      const exportable = groups.filter((g) => g.mode !== "background");
      const data =
        exportable.length > 0
          ? build3MFColored(positions, exportable)
          : build3MF(pos);
      saveExport(data, "3mf");
      return;
    }
    const data =
      format === "off"
        ? buildOFF(pos)
        : format === "obj"
          ? buildOBJ(pos)
          : format === "amf"
            ? buildAMF(pos)
            : buildBinarySTL(pos);
    saveExport(data, format);
  }

  // Keep the imperative refs (editor keymap, native menu) pointing at the latest
  // closures so they never see stale state.
  saveActiveRef.current = () => void saveActive(false);
  saveAsRef.current = () => void saveActive(true);
  menuExportRef.current = () => void onDownload(exportFmt);
  highlightFromCursorRef.current = highlightFromCursor;

  // Command registry (⌘K). Web actions only — the desktop native menu is a
  // separate Rust-driven surface and isn't unified here.
  const commands: Command[] = [
    {
      id: "render",
      title: "Render",
      shortcut: "⌘↵",
      run: () => renderNowRef.current(),
    },
    {
      id: "stop",
      title: "Stop render",
      when: rendering,
      run: () => engineRef.current?.cancel(),
    },
    {
      id: "fit",
      title: "Zoom to fit",
      shortcut: "⌘⇧F",
      run: () => viewerRef.current?.fit(),
    },
    {
      id: "reset-view",
      title: "Reset view",
      run: () => viewerRef.current?.resetView(),
    },
    {
      id: "console",
      title: "Toggle console",
      shortcut: "⌘J",
      run: () => setConsoleOpen((o) => !o),
    },
    { id: "dock", title: "Toggle dock", run: toggleDock },
    {
      id: "grid",
      title: "Toggle grid & axes",
      run: () => toggleGrid(!showGrid),
    },
    {
      id: "edges",
      title: "Toggle edge overlay",
      run: () => toggleEdges(!showEdges),
    },
    { id: "fast", title: "Toggle fast preview", run: toggleFastPreview },
    {
      id: "engine",
      title: `Switch engine (${engineKind === "openscad" ? "→ Quito" : "→ OpenSCAD"})`,
      run: toggleEngine,
    },
    {
      id: "q-draft",
      title: "Quality: Draft",
      run: () => setQualityPref({ quality: "draft" }),
    },
    {
      id: "q-normal",
      title: "Quality: Normal",
      run: () => setQualityPref({ quality: "normal" }),
    },
    {
      id: "q-fine",
      title: "Quality: Fine",
      run: () => setQualityPref({ quality: "fine" }),
    },
    { id: "png", title: "Save PNG", run: () => void onSavePng() },
    {
      id: "export",
      title: `Export (${exportFmt.toUpperCase()})`,
      run: () => void onDownload(exportFmt),
    },
    {
      id: "help",
      title: "Help & keyboard shortcuts",
      run: () => setHelpOpen(true),
    },
    { id: "new", title: "New project", run: newProject },
    ...(TAURI
      ? []
      : [{ id: "share", title: "Copy share link", run: () => void onShare() }]),
  ];

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          Quito <span className="tag">playground</span>
        </div>
        <div className="actions">
          <button onClick={newProject}>New</button>
          <select
            className="examples-select"
            aria-label="Load example"
            value=""
            onChange={(e) => {
              const i = Number(e.target.value);
              if (e.target.value !== "") loadExample(i);
            }}
          >
            <option value="" disabled>
              Examples…
            </option>
            {EXAMPLES.map((ex, i) => (
              <option key={i} value={i}>
                {ex.label}
              </option>
            ))}
          </select>
          {TAURI && <button onClick={openNative}>Open…</button>}
          {TAURI && (
            <button
              onClick={() => saveActiveRef.current()}
              title="Save the active file (⌘S)"
            >
              Save
            </button>
          )}
          {!TAURI && (
            <button
              onClick={onShare}
              title="Copy a shareable link to this project"
            >
              {shareMsg || "Share"}
            </button>
          )}
          {!TAURI && (
            <button
              onClick={onDownloadScad}
              title="Download the active file as .scad"
            >
              .scad
            </button>
          )}
          <Popover
            label="Display"
            title="Viewport display options"
            active={ortho || !showGrid || !showEdges || !linkHighlight}
          >
            <PopoverToggle checked={ortho} onChange={setOrthoProjection}>
              Orthographic projection
            </PopoverToggle>
            <PopoverToggle
              checked={linkHighlight}
              onChange={() => toggleLinkHighlight()}
            >
              Link editor ↔ preview
            </PopoverToggle>
            <PopoverToggle checked={showGrid} onChange={toggleGrid}>
              Grid &amp; axes
            </PopoverToggle>
            <PopoverToggle checked={showEdges} onChange={toggleEdges}>
              Edge overlay
            </PopoverToggle>
            <PopoverToggle checked={showDims} onChange={toggleDims}>
              Dimensions
            </PopoverToggle>
          </Popover>
          <button
            className={engineKind === "openscad" ? "active" : undefined}
            onClick={toggleEngine}
            title={
              engineKind === "openscad"
                ? TAURI
                  ? "Rendering with OpenSCAD (Manifold) — your locally-installed OpenSCAD if available, otherwise the bundled wasm build. Click to switch back to Quito."
                  : "Rendering with OpenSCAD 2025.03.25 (Manifold) — the vendored OpenSCAD wasm engine. Click to switch back to Quito."
                : TAURI
                  ? "Rendering with Quito — our native engine. Click to switch to OpenSCAD (uses your local install if available, else the bundled wasm build)."
                  : "Rendering with Quito — our engine. Click to switch to the OpenSCAD wasm engine (first use downloads ~10 MB)."
            }
          >
            {engineKind === "openscad" ? "OpenSCAD" : "Quito"}
          </button>
          <button
            className={fastPreview ? "active" : undefined}
            onClick={toggleFastPreview}
            title={
              engineKind === "openscad"
                ? fastPreview
                  ? "Preview on — OpenSCAD F5-style colored render (shows color(...)). Click for a plain exact render."
                  : "Preview off — plain exact (F6-style) render. Click for an F5-style colored preview."
                : fastPreview
                  ? "Fast preview on — unions are skipped (not watertight); much faster to render. Exports & volume stay exact. Click to disable."
                  : "Fast preview off — exact, watertight render. Click to enable a faster, non-watertight preview."
            }
          >
            Fast
          </button>
          <select
            className="quality-select"
            aria-label="Render quality"
            value={quality}
            onChange={(e) =>
              setQualityPref({ quality: e.target.value as Quality })
            }
            title="Render resolution ($fn/$fa/$fs). Draft is coarse and fast; Fine is smooth and slow; Normal respects the script."
          >
            <option value="draft">Draft</option>
            <option value="normal">Normal</option>
            <option value="fine">Fine</option>
            <option value="custom">Custom</option>
          </select>
          {quality === "custom" && (
            <label
              className="quality-fn"
              title="Custom $fn (blank = leave to $fa/$fs)"
            >
              $fn
              <input
                type="number"
                min={0}
                step={1}
                value={customFn ?? ""}
                onChange={(e) =>
                  setQualityPref({
                    customFn:
                      e.target.value === ""
                        ? null
                        : Math.max(0, Math.round(Number(e.target.value))),
                  })
                }
              />
            </label>
          )}
          <button
            onClick={onSavePng}
            title="Save the current view as a PNG image"
          >
            PNG
          </button>
          <div className="anim" title="Animation ($t sweeps 0→1)">
            <button
              className="anim-play"
              onClick={() => setPlaying((p) => !p)}
              title={playing ? "Pause animation" : "Play animation"}
              aria-label={playing ? "Pause animation" : "Play animation"}
            >
              {playing ? "⏸" : "▶"}
            </button>
            <input
              type="range"
              min={0}
              max={1}
              step={0.001}
              value={time}
              onChange={(e) => seekTime(parseFloat(e.target.value))}
              title="Animation time $t (0–1)"
            />
            <span className="anim-val" title="Current $t">
              {time.toFixed(3)}
            </span>
            <label className="anim-field" title="Frames per second">
              FPS
              <input
                type="number"
                min={1}
                max={60}
                value={fps}
                onChange={(e) =>
                  setFps(
                    Math.max(
                      1,
                      Math.min(60, Math.round(parseFloat(e.target.value) || 1)),
                    ),
                  )
                }
              />
            </label>
            <label
              className="anim-field"
              title="Number of frames as $t goes 0→1"
            >
              Steps
              <input
                type="number"
                min={1}
                max={1000}
                value={steps}
                onChange={(e) =>
                  setSteps(
                    Math.max(
                      1,
                      Math.min(
                        1000,
                        Math.round(parseFloat(e.target.value) || 1),
                      ),
                    ),
                  )
                }
              />
            </label>
            <button
              onClick={onExportFrames}
              disabled={status.triangleCount === 0}
              title="Render every frame and download a zip of PNGs"
            >
              Frames
            </button>
          </div>
          <div className="export">
            <button
              onClick={() => onDownload(exportFmt)}
              disabled={status.triangleCount === 0}
            >
              Export
            </button>
            <select
              aria-label="Export format"
              value={exportFmt}
              onChange={(e) => {
                userPickedFmtRef.current = true;
                setExportFmt(e.target.value as ExportFmt);
              }}
            >
              {(is2D ? FORMATS_2D : FORMATS_3D).map((f) => (
                <option key={f} value={f}>
                  {f.toUpperCase()}
                </option>
              ))}
            </select>
          </div>
          <button
            className="cmdk"
            onClick={() => setPaletteOpen(true)}
            title="Command palette (⌘K)"
            aria-label="Open command palette"
          >
            ⌘K
          </button>
          <button
            className="help-btn"
            onClick={() => setHelpOpen(true)}
            title="Help & keyboard shortcuts"
            aria-label="Help"
          >
            ?
          </button>
          <button
            className="github-link"
            onClick={() => openExternal(GITHUB_URL)}
            title="View source on GitHub"
            aria-label="View source on GitHub"
          >
            <svg viewBox="0 0 16 16" width="18" height="18" aria-hidden="true">
              <path
                fill="currentColor"
                fillRule="evenodd"
                d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8z"
              />
            </svg>
          </button>
        </div>
      </header>

      {saveFailed && (
        <div className="update-banner error" role="alert">
          <div className="update-banner-row">
            <span className="update-banner-msg">
              Browser storage is full — your work is no longer being autosaved
              and will be lost on reload. Export the file, or free up space.
            </span>
            <div className="update-banner-actions">
              <button
                className="update-dismiss"
                onClick={() => setSaveFailed(false)}
                aria-label="Dismiss"
              >
                ✕
              </button>
            </div>
          </div>
        </div>
      )}

      {recovering && (
        <div className="update-banner error recovery-banner" role="alert">
          <div className="update-banner-row">
            <span className="update-banner-msg">
              The last render didn't finish — this model may be too heavy and
              could freeze the app. Your script is loaded but not rendered.
              Simplify it (e.g. lower <code>$fn</code>
              ), or render anyway.
            </span>
            <div className="update-banner-actions">
              <button
                className="update-primary"
                onClick={() => {
                  setRecovering(false);
                  renderNowRef.current();
                }}
              >
                Render anyway
              </button>
              <button
                className="update-dismiss"
                onClick={() => setRecovering(false)}
                aria-label="Dismiss"
              >
                ✕
              </button>
            </div>
          </div>
        </div>
      )}

      {TAURI && (
        <UpdateBanner
          state={updater.state}
          onInstall={() => void updater.startInstall()}
          onDismiss={updater.dismiss}
        />
      )}

      <div
        className="workspace"
        style={{
          gridTemplateColumns: `${effEditorW}px 6px 1fr ${
            dockCollapsed ? "28px" : `6px ${effDockW}px`
          }`,
        }}
      >
        <div className="editor-col">
          <div className="tabs">
            {files.map((f, i) => {
              const dirty =
                TAURI && !!f.path && f.content !== savedRef.current[f.name];
              // The engine reports errors/warnings against the main file; when
              // it's not the active tab, badge it so the squiggles aren't missed.
              const diagKind =
                i === 0 && active !== 0
                  ? diagCounts.errors > 0
                    ? "error"
                    : diagCounts.warnings > 0
                      ? "warn"
                      : ""
                  : "";
              return (
                <div
                  key={i}
                  className={`tab ${i === active ? "active" : ""}`}
                  onClick={() => switchTo(i)}
                  onDoubleClick={() => renameFile(i)}
                  title={i === 0 ? "main (rendered)" : "double-click to rename"}
                >
                  {diagKind && (
                    <span
                      className={`tab-diag ${diagKind}`}
                      title={
                        diagKind === "error"
                          ? "Errors in this file"
                          : "Warnings in this file"
                      }
                      aria-label={diagKind === "error" ? "Errors" : "Warnings"}
                    >
                      ●
                    </span>
                  )}
                  {dirty && (
                    <span
                      className="tab-dirty"
                      title="Unsaved changes"
                      aria-label="Unsaved changes"
                    >
                      ●
                    </span>
                  )}
                  <span>{f.name}</span>
                  {i > 0 && (
                    <button
                      className="tab-close"
                      onClick={(e) => {
                        e.stopPropagation();
                        deleteFile(i);
                      }}
                      title="Delete file"
                    >
                      ×
                    </button>
                  )}
                </div>
              );
            })}
            <button className="tab-add" onClick={addFile} title="Add file">
              +
            </button>
          </div>
          <div className="editor" ref={editorHost} />
        </div>
        <ResizeHandle
          axis="x"
          onDelta={dragEditor}
          onCommit={persistSizes}
          title="Drag to resize the editor"
        />
        <div className="viewer">
          <canvas ref={canvasRef} />
          <button
            className="viewer-fit"
            onClick={() => viewerRef.current?.fit()}
            title="Zoom to fit — frame the model without changing the angle (⌘⇧F)"
            aria-label="Zoom to fit"
          >
            ⤢ Fit
          </button>
        </div>
        {!dockCollapsed && (
          <ResizeHandle
            axis="x"
            onDelta={dragDock}
            onCommit={persistSizes}
            title="Drag to resize the dock"
          />
        )}
        <Dock
          params={schema}
          overrides={overrides}
          onChange={setOverride}
          onReset={resetOverrides}
          presets={Object.keys(paramSets)}
          onApplyPreset={applyPreset}
          onSavePreset={savePreset}
          onDeletePreset={deletePreset}
          onImportPresets={importPresets}
          onExportPresets={exportPresets}
          model={{
            ok: status.ok,
            triangleCount: status.triangleCount,
            vertexCount: status.vertexCount,
            area: status.area,
            volume: status.volume,
            preview: status.preview,
            geomErrors: status.geomErrors,
            dims,
            groups: lastPreview.current.groups,
            libraries: files.slice(1).map((f) => f.name),
          }}
          collapsed={dockCollapsed}
          onToggleCollapsed={toggleDock}
          paramsOpen={paramsOpen}
          onToggleParams={toggleParamsSection}
          modelOpen={modelOpen}
          onToggleModel={toggleModelSection}
        />
      </div>

      {consoleOpen && (
        <ResizeHandle
          axis="y"
          onDelta={dragConsole}
          onCommit={persistSizes}
          title="Drag to resize the console"
        />
      )}
      {consoleOpen &&
        (() => {
          const counts = {
            error: consoleLines.filter((l) => l.kind === "error").length,
            warn: consoleLines.filter((l) => l.kind === "warn").length,
            echo: consoleLines.filter((l) => l.kind === "echo").length,
          };
          const shown = consoleLines.filter(
            (l) => consoleFilter === "all" || l.kind === consoleFilter,
          );
          const chip = (
            key: "all" | "error" | "warn" | "echo",
            label: string,
          ) => (
            <button
              className={`console-chip ${key} ${
                consoleFilter === key ? "active" : ""
              }`}
              onClick={() => setConsoleFilter(key)}
            >
              {label}
            </button>
          );
          return (
            <div className="console" style={{ height: effConsoleH }}>
              <div className="console-filters">
                {chip("all", "All")}
                {chip("error", `Errors ${counts.error}`)}
                {chip("warn", `Warnings ${counts.warn}`)}
                {chip("echo", `Echo ${counts.echo}`)}
              </div>
              <div className="console-body">
                {shown.length === 0 ? (
                  <div className="console-line muted">No output.</div>
                ) : (
                  shown.map((l, i) =>
                    l.span ? (
                      <button
                        className={`console-line ${l.kind} clickable`}
                        key={i}
                        onClick={() => jumpToSpan(l.span!)}
                        title="Jump to source"
                      >
                        {l.text}
                      </button>
                    ) : (
                      <div className={`console-line ${l.kind}`} key={i}>
                        {l.text}
                      </div>
                    ),
                  )
                )}
              </div>
            </div>
          );
        })()}

      <footer
        className={`statusbar ${status.ok ? "ok" : "err"}`}
        data-render-rev={renderRev}
      >
        {/* Fixed-width control cell so Render↔(rendering… Stop) can't shift the
            numbers sideways every render. */}
        <span className="status-controls">
          {rendering ? (
            <>
              <span className="status-rendering">rendering…</span>
              <button
                className="status-stop"
                onClick={() => engineRef.current?.cancel()}
                title="Stop the current render"
              >
                Stop
              </button>
            </>
          ) : (
            <button
              className="status-render"
              onClick={() => renderNowRef.current()}
              title="Render the current model"
            >
              Render
            </button>
          )}
        </span>
        <span className="status-main">{status.message}</span>
        {/* Hold the last-good numbers across renders (don't gate on !rendering)
            so they don't blink ~15×/s during animation playback. */}
        {status.ok && (
          <span className="status-meta">
            {dims &&
              `${fmtDim(dims.x)} × ${fmtDim(dims.y)} × ${fmtDim(dims.z)} mm · `}
            {status.preview ? (
              <span title="Fast preview is on: unions are skipped, so volume is approximate. Turn off Fast (or export) for the exact value.">
                vol ≈ {status.volume.toFixed(2)} (preview)
              </span>
            ) : (
              <>vol {status.volume.toFixed(2)}</>
            )}{" "}
            · {status.ms.toFixed(0)} ms
          </span>
        )}
        {status.ok && (
          <span
            className={`status-integrity ${
              status.geomErrors
                ? "degraded"
                : status.preview
                  ? "preview"
                  : "exact"
            }`}
            title={
              status.geomErrors
                ? "Degraded: a CSG op failed and a fallback mesh is shown — the geometry is not trustworthy."
                : status.preview
                  ? "Fast preview: unions are skipped, so the mesh isn't watertight and the volume is approximate. Exports re-render exact."
                  : "Exact: watertight geometry; the numbers are trustworthy."
            }
          >
            {status.geomErrors
              ? "DEGRADED"
              : status.preview
                ? "FAST PREVIEW"
                : "EXACT"}
          </span>
        )}
        <button
          className={`console-toggle ${consoleLines.some((l) => l.kind !== "echo") ? "alert" : ""}`}
          onClick={() => setConsoleOpen((o) => !o)}
          title="Toggle console"
        >
          console{consoleLines.length ? ` (${consoleLines.length})` : ""}
        </button>
        <span className="status-version">{version && `engine ${version}`}</span>
      </footer>

      {paletteOpen && (
        <CommandPalette
          commands={commands}
          onClose={() => setPaletteOpen(false)}
        />
      )}
      {helpOpen && <HelpSheet onClose={() => setHelpOpen(false)} />}
    </div>
  );
}
