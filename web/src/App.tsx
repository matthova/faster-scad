import { useEffect, useRef, useState } from "react";
import { EditorView, keymap } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { setDiagnostics, type Diagnostic } from "@codemirror/lint";
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { openscad } from "./lang/openscad";
import { Viewer, type MeshInfo, type ViewPreset, type PreviewGroup } from "./viewer";
import { Engine, export2dBrowser } from "./engine";
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
import { CustomizerPanel } from "./CustomizerPanel";
import {
  parseSchema,
  toLiteral,
  toParamSetsJson,
  fromParamSetsJson,
  type Param,
  type ParamValue,
} from "./customizer";
import { loadProject, saveProject, clearProject, type File } from "./project";
import { EXAMPLES } from "./examples";
import { decodeSharedProject, shareUrl } from "./share";
import { resolveClosure } from "./library";
import {
  isTauri,
  DesktopEngine,
  saveModelNative,
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
import { checkForUpdates } from "./checkForUpdates";

const TAURI = isTauri();

// Base URL for bundled libraries (public/lib/…), resolved against the page.
const LIB_BASE = new URL("lib/", document.baseURI).href;

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

interface Status {
  ok: boolean;
  message: string;
  triangleCount: number;
  volume: number;
  ms: number;
  echo: string;
  warnings: string;
  error: string;
}

export function App() {
  const editorHost = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewerRef = useRef<Viewer | null>(null);
  const engineRef = useRef<Engine | DesktopEngine | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const lastPositions = useRef<Float32Array>(new Float32Array(0));
  // Preview color channel from the last render, for colored 3MF export.
  const lastPreview = useRef<{ positions: Float32Array; groups: PreviewGroup[] }>({
    positions: new Float32Array(0),
    groups: [],
  });
  const debounceTimer = useRef<number | undefined>(undefined);

  // File + customizer state. A `#code/…` share link (browser only) wins over
  // the autosaved localStorage project, so opening a shared URL always shows
  // that project. Refs mirror state so imperative render/edit paths never see a
  // stale closure.
  const sharedRef = useRef(TAURI ? null : decodeSharedProject());
  const saved = useRef(sharedRef.current ?? loadProject()).current;
  const filesRef = useRef<File[]>(saved?.files ?? DEFAULT_FILES.map((f) => ({ ...f })));
  const activeRef = useRef(saved?.active ?? 0);
  const suppressRef = useRef(false);
  const overridesRef = useRef<Record<string, ParamValue>>(saved?.overrides ?? {});
  const paramSetsRef = useRef<Record<string, Record<string, ParamValue>>>(saved?.paramSets ?? {});
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
  // Latest engine diagnostics (for the main file) — squiggled in the editor when
  // the main tab is active, and badged on the tab otherwise.
  const diagRef = useRef<EngineDiag[]>([]);
  // Animation playback: a share link may carry $t/fps/steps/play-state so the
  // recipient opens on the same frame and speed.
  const sharedAnim = sharedRef.current?.anim;
  const timeRef = useRef(sharedAnim?.t ?? 0); // $t for animation
  const stepRef = useRef(Math.round((sharedAnim?.t ?? 0) * (sharedAnim?.steps ?? 20))); // current animation frame index (0..steps-1)

  const [files, setFiles] = useState<File[]>(filesRef.current);
  const [active, setActive] = useState(activeRef.current);
  const [status, setStatus] = useState<Status>({
    ok: true,
    message: "initializing…",
    triangleCount: 0,
    volume: 0,
    ms: 0,
    echo: "",
    warnings: "",
    error: "",
  });
  const [version, setVersion] = useState("");
  const [consoleOpen, setConsoleOpen] = useState(false);
  const [exportFmt, setExportFmt] = useState<ExportFmt>("stl");
  const [is2D, setIs2D] = useState(false);
  const [dims, setDims] = useState<MeshInfo | null>(null);
  const [ortho, setOrtho] = useState(false);
  const [time, setTime] = useState(sharedAnim?.t ?? 0);
  const [playing, setPlaying] = useState(sharedAnim?.playing ?? false);
  const [fps, setFps] = useState(sharedAnim?.fps ?? 15);
  const [steps, setSteps] = useState(sharedAnim?.steps ?? 20);
  const [schema, setSchema] = useState<Param[]>([]);
  const [overrides, setOverrides] = useState<Record<string, ParamValue>>(overridesRef.current);
  const [paramSets, setParamSets] = useState<Record<string, Record<string, ParamValue>>>(
    paramSetsRef.current,
  );
  const [shareMsg, setShareMsg] = useState("");
  // Diagnostic counts for the main file (error/warning), for the tab badge.
  const [diagCounts, setDiagCounts] = useState<{ errors: number; warnings: number }>({
    errors: 0,
    warnings: 0,
  });

  function persist() {
    saveProject({
      files: filesRef.current,
      overrides: overridesRef.current,
      active: activeRef.current,
      paramSets: paramSetsRef.current,
    });
  }

  useEffect(() => {
    if (!canvasRef.current || !editorHost.current) return;

    const viewer = new Viewer(canvasRef.current, (info) => setDims(info));
    viewerRef.current = viewer;

    const engine = TAURI
      ? new DesktopEngine((r: RenderResponse) => onResult(r))
      : new Engine((r: RenderResponse) => onResult(r));
    engineRef.current = engine;

    const renderNow = async () => {
      const fs = filesRef.current;
      const ov = overridesRef.current;
      const names = Object.keys(ov);
      const values = names.map((n) => toLiteral(ov[n]));
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
      if (TAURI) {
        // Native engine resolves include/use from disk (OPENSCADPATH) + the
        // in-memory tabs; no CDN fetch needed.
        engine.render(
          fs[0].content,
          names,
          values,
          libs.map((f) => f.name),
          libs.map((f) => f.content),
        );
      } else {
        // Browser: resolve the include/use closure (fetching libraries), then
        // render with the full file set.
        const { names: fileNames, contents: fileContents } = await resolveClosure(
          fs[0].content,
          libs,
          LIB_BASE,
        );
        engine.render(fs[0].content, names, values, fileNames, fileContents);
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
      if (filesRef.current[0].content.includes("$vp")) requestRenderRef.current();
    });

    const view = new EditorView({
      state: EditorState.create({
        doc: filesRef.current[activeRef.current].content,
        extensions: [
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
          EditorView.theme({
            "&": { height: "100%", fontSize: "13px" },
            ".cm-scroller": { fontFamily: "ui-monospace, Menlo, monospace" },
          }),
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
          }),
        ],
      }),
      parent: editorHost.current,
    });
    viewRef.current = view;

    renderNow(); // initial render

    // A project opened from a share link isn't in localStorage yet — persist it
    // now so a plain reload (or losing the hash) keeps the shared work.
    if (sharedRef.current) persist();

    // Desktop wiring: external-edit reload, native menu, and open-with.
    const unlisteners: (() => void)[] = [];
    if (TAURI) {
      // Seed saved baselines for any restored files that already have a disk path,
      // and (re)arm watchers for them so external edits reload after a relaunch.
      const paths = filesRef.current.map((f) => f.path).filter((p): p is string => !!p);
      for (const f of filesRef.current) if (f.path) savedRef.current[f.name] = f.content;
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
            void checkForUpdates(true);
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

      // Silent update check on launch: prompts only if an update is available,
      // stays quiet on "up to date" and on errors (offline, etc.).
      void checkForUpdates(false);
    }

    return () => {
      view.destroy();
      unsubCamera();
      for (const u of unlisteners) u();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
  function setMainFile(name: string, content: string, dir?: string, path?: string) {
    const next = filesRef.current.slice();
    next[0] = { name, content, path: path ?? next[0].path };
    filesRef.current = next;
    setFiles(next);
    if (path) savedRef.current[name] = content;
    if (dir && engineRef.current instanceof DesktopEngine) engineRef.current.dir = dir;
    if (activeRef.current === 0 && viewRef.current) {
      const view = viewRef.current;
      suppressRef.current = true;
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: content } });
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
      setMainFile(filesRef.current[0].name, content, undefined, filesRef.current[0].path);
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
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: content } });
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
        // Main file's directory drives include/use resolution on the native engine.
        if (idx === 0 && engineRef.current instanceof DesktopEngine) {
          engineRef.current.dir = path.slice(0, path.length - basename(path).length) || ".";
        }
        void watchFiles(filesRef.current.map((x) => x.path).filter((p): p is string => !!p));
      }
    } catch (e) {
      setStatus((s) => ({ ...s, ok: false, error: `save failed: ${String(e)}`, message: "save failed" }));
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

  function switchTo(idx: number) {
    if (idx === activeRef.current || !viewRef.current) return;
    activeRef.current = idx;
    setActive(idx);
    const view = viewRef.current;
    suppressRef.current = true;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: filesRef.current[idx].content },
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
      window.history.replaceState(null, "", window.location.pathname + window.location.search);
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
    if (!window.confirm(`Load the "${ex.label}" example? This replaces the current project.`))
      return;
    sharedRef.current = null;
    try {
      window.history.replaceState(null, "", window.location.pathname + window.location.search);
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
    if (!name || name === cur || filesRef.current.some((f) => f.name === name)) return;
    const next = filesRef.current.slice();
    next[idx] = { ...next[idx], name };
    filesRef.current = next;
    setFiles(next);
    persist();
    requestRenderRef.current();
  }

  function onResult(r: RenderResponse) {
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
      if (Object.keys(kept).length !== Object.keys(overridesRef.current).length) {
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
        viewerRef.current?.setColoredMesh(r.previewPositions, r.previewNormals, groups);
      } else {
        viewerRef.current?.setMesh(r.positions, r.normals);
      }
      // A script that assigned `$vp*` drives the camera: apply it when the
      // returned viewport differs from the camera we sent.
      if (r.viewport && viewerRef.current && !exportingRef.current) {
        applyScriptCamera(r.viewport);
      }
      // Offer vector formats for 2D models, mesh formats for 3D; keep the
      // selected format valid when the model's dimensionality changes.
      setIs2D(r.is2D);
      setExportFmt((f) =>
        r.is2D
          ? FORMATS_2D.includes(f)
            ? f
            : "dxf"
          : FORMATS_3D.includes(f)
            ? f
            : "stl",
      );
      setStatus({
        ok: true,
        message: `${r.triangleCount.toLocaleString()} triangles`,
        triangleCount: r.triangleCount,
        volume: r.volume,
        ms: r.ms,
        echo: r.echo,
        warnings: r.warnings,
        error: "",
      });
    } else {
      setStatus((s) => ({
        ...s,
        ok: false,
        message: r.error,
        ms: r.ms,
        echo: r.echo,
        warnings: r.warnings,
        error: r.error,
      }));
      setConsoleOpen(true);
    }

    if (frameWaiter) {
      frameWaiterRef.current = null;
      frameWaiter();
    }
  }

  const consoleLines: { kind: "error" | "warn" | "echo"; text: string }[] = [];
  if (status.error) consoleLines.push({ kind: "error", text: status.error });
  for (const w of status.warnings.split("\n").filter(Boolean))
    consoleLines.push({ kind: "warn", text: `WARNING: ${w}` });
  for (const e of status.echo.split("\n").filter(Boolean))
    consoleLines.push({ kind: "echo", text: e });

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
    for (const p of schema) snapshot[p.name] = overridesRef.current[p.name] ?? p.value;
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
    const nearN = (a: number | null | undefined, b: number) => a == null || Math.abs(a - b) < 1e-3;
    const nearV = (a: [number, number, number] | null | undefined, b: [number, number, number]) =>
      !a || a.every((x, i) => Math.abs(x - b[i]) < 1e-3);
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

    if (TAURI) {
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
        const { names: fileNames, contents: fileContents } = await resolveClosure(
          fs[0].content,
          libs,
          LIB_BASE,
        );
        const text = await export2dBrowser({
          source: fs[0].content,
          names,
          values,
          fileNames,
          fileContents,
          format,
        });
        downloadBlob(new TextEncoder().encode(text), `quito.${format}`);
      } catch (err) {
        setStatus((s) => ({ ...s, error: `export failed: ${String(err)}` }));
        setConsoleOpen(true);
      }
      return;
    }

    // 3D mesh formats: build client-side from the last render soup.
    const pos = lastPositions.current;
    if (pos.length === 0) return;
    // Colored 3MF: one object per non-`%` color group (falls back to fused 3MF).
    if (format === "3mf") {
      const { positions, groups } = lastPreview.current;
      const exportable = groups.filter((g) => g.mode !== "background");
      const data =
        exportable.length > 0 ? build3MFColored(positions, exportable) : build3MF(pos);
      downloadBlob(data, `quito.3mf`);
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
    downloadBlob(data, `quito.${format}`);
  }

  // Keep the imperative refs (editor keymap, native menu) pointing at the latest
  // closures so they never see stale state.
  saveActiveRef.current = () => void saveActive(false);
  saveAsRef.current = () => void saveActive(true);
  menuExportRef.current = () => void onDownload(exportFmt);

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
            <button onClick={() => saveActiveRef.current()} title="Save the active file (⌘S)">
              Save
            </button>
          )}
          {!TAURI && (
            <button onClick={onShare} title="Copy a shareable link to this project">
              {shareMsg || "Share"}
            </button>
          )}
          {!TAURI && (
            <button onClick={onDownloadScad} title="Download the active file as .scad">
              .scad
            </button>
          )}
          <span className="view-presets">
            {(["iso", "front", "top", "right"] as ViewPreset[]).map((p) => (
              <button
                key={p}
                onClick={() => viewerRef.current?.setPreset(p)}
                title={`${p} view`}
              >
                {p[0].toUpperCase()}
              </button>
            ))}
          </span>
          <button onClick={() => viewerRef.current?.resetView()}>Reset view</button>
          <button
            className={ortho ? "active" : undefined}
            onClick={() => {
              const next = !ortho;
              viewerRef.current?.setProjection(next ? "orthographic" : "perspective");
              setOrtho(next);
            }}
            title={
              ortho
                ? "Orthographic projection (parallel) — click for perspective"
                : "Perspective projection — click for orthographic (isometric)"
            }
          >
            {ortho ? "Ortho" : "Persp"}
          </button>
          <button onClick={onSavePng} title="Save the current view as a PNG image">
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
                  setFps(Math.max(1, Math.min(60, Math.round(parseFloat(e.target.value) || 1))))
                }
              />
            </label>
            <label className="anim-field" title="Number of frames as $t goes 0→1">
              Steps
              <input
                type="number"
                min={1}
                max={1000}
                value={steps}
                onChange={(e) =>
                  setSteps(Math.max(1, Math.min(1000, Math.round(parseFloat(e.target.value) || 1))))
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
            <button onClick={() => onDownload(exportFmt)} disabled={status.triangleCount === 0}>
              Export
            </button>
            <select
              aria-label="Export format"
              value={exportFmt}
              onChange={(e) => setExportFmt(e.target.value as ExportFmt)}
            >
              {(is2D ? FORMATS_2D : FORMATS_3D).map((f) => (
                <option key={f} value={f}>
                  {f.toUpperCase()}
                </option>
              ))}
            </select>
          </div>
        </div>
      </header>

      <div className="workspace">
        <div className="editor-col">
          <div className="tabs">
            {files.map((f, i) => {
              const dirty = TAURI && !!f.path && f.content !== savedRef.current[f.name];
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
                    title={diagKind === "error" ? "Errors in this file" : "Warnings in this file"}
                    aria-label={diagKind === "error" ? "Errors" : "Warnings"}
                  >
                    ●
                  </span>
                )}
                {dirty && (
                  <span className="tab-dirty" title="Unsaved changes" aria-label="Unsaved changes">
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
        <div className="viewer">
          <canvas ref={canvasRef} />
        </div>
        <CustomizerPanel
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
        />
      </div>

      {consoleOpen && (
        <div className="console">
          {consoleLines.length === 0 ? (
            <div className="console-line muted">No output.</div>
          ) : (
            consoleLines.map((l, i) => (
              <div className={`console-line ${l.kind}`} key={i}>
                {l.text}
              </div>
            ))
          )}
        </div>
      )}

      <footer className={`statusbar ${status.ok ? "ok" : "err"}`}>
        <span className="status-main">{status.message}</span>
        {status.ok && (
          <span className="status-meta">
            {dims && `${fmtDim(dims.x)} × ${fmtDim(dims.y)} × ${fmtDim(dims.z)} mm · `}
            vol {status.volume.toFixed(2)} · {status.ms.toFixed(0)} ms
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
    </div>
  );
}
