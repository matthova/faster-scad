import { useEffect, useRef, useState } from "react";
import { EditorView, keymap } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { openscad } from "./lang/openscad";
import { Viewer } from "./viewer";
import { Engine } from "./engine";
import type { RenderResponse } from "./engineWorker";
import { buildBinarySTL, buildOFF, buildOBJ, downloadBlob } from "./stl";
import { CustomizerPanel } from "./CustomizerPanel";
import { parseSchema, toLiteral, type Param, type ParamValue } from "./customizer";
import { loadProject, saveProject, clearProject, type File } from "./project";
import { decodeSharedProject, shareUrl } from "./share";
import { resolveClosure } from "./library";
import {
  isTauri,
  DesktopEngine,
  saveModelNative,
  openScadFile,
  onFileChanged,
} from "./desktopEngine";

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
  const paramsJsonRef = useRef("");
  const requestRenderRef = useRef<() => void>(() => {});
  const timeRef = useRef(0); // $t for animation

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
  const [exportFmt, setExportFmt] = useState<"stl" | "off" | "obj">("stl");
  const [time, setTime] = useState(0);
  const [schema, setSchema] = useState<Param[]>([]);
  const [overrides, setOverrides] = useState<Record<string, ParamValue>>(overridesRef.current);
  const [shareMsg, setShareMsg] = useState("");

  function persist() {
    saveProject({
      files: filesRef.current,
      overrides: overridesRef.current,
      active: activeRef.current,
    });
  }

  useEffect(() => {
    if (!canvasRef.current || !editorHost.current) return;

    const viewer = new Viewer(canvasRef.current);
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

    const view = new EditorView({
      state: EditorState.create({
        doc: filesRef.current[activeRef.current].content,
        extensions: [
          basicSetup,
          keymap.of([indentWithTab]),
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

    // Live-reload the main file when it's edited in an external editor (desktop).
    let unlisten: (() => void) | undefined;
    if (TAURI) {
      onFileChanged(({ content }) => setMainFile(filesRef.current[0].name, content))
        .then((u) => (unlisten = u))
        .catch(() => {});
    }

    return () => {
      view.destroy();
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Replace the rendered (first) file's content — from a native open or an
   *  external-edit reload — updating the editor if that tab is active. */
  function setMainFile(name: string, content: string, dir?: string) {
    const next = filesRef.current.slice();
    next[0] = { name, content };
    filesRef.current = next;
    setFiles(next);
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

  async function openNative() {
    try {
      const f = await openScadFile();
      if (f) setMainFile(f.name, f.content, f.dir);
    } catch {
      /* dialog cancelled / unavailable */
    }
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
    view.focus();
    persist();
  }

  async function onShare() {
    const url = shareUrl({
      files: filesRef.current,
      overrides: overridesRef.current,
      active: activeRef.current,
    });
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
      viewerRef.current?.setMesh(r.positions, r.normals);
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

  function onDownload(format: "stl" | "off" | "obj") {
    if (status.triangleCount === 0) return;
    if (TAURI) {
      // Native: re-render on the native engine and write via a save dialog, so
      // the exported mesh is welded/exact (not derived from the render soup).
      const fs = filesRef.current;
      const ov = overridesRef.current;
      const names = Object.keys(ov);
      const libs = fs.slice(1);
      void saveModelNative(
        format,
        fs[0].content,
        names,
        names.map((n) => toLiteral(ov[n])),
        libs.map((f) => f.name),
        libs.map((f) => f.content),
      );
      return;
    }
    const pos = lastPositions.current;
    if (pos.length === 0) return;
    const data =
      format === "off" ? buildOFF(pos) : format === "obj" ? buildOBJ(pos) : buildBinarySTL(pos);
    downloadBlob(data, `quito.${format}`);
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          Quito <span className="tag">playground</span>
        </div>
        <div className="actions">
          <button onClick={newProject}>New</button>
          {TAURI && <button onClick={openNative}>Open…</button>}
          {!TAURI && (
            <button onClick={onShare} title="Copy a shareable link to this project">
              {shareMsg || "Share"}
            </button>
          )}
          <button onClick={() => viewerRef.current?.resetView()}>Reset view</button>
          <label className="anim" title="Animation time $t (0–1)">
            $t
            <input
              type="range"
              min={0}
              max={1}
              step={0.01}
              value={time}
              onChange={(e) => {
                const t = parseFloat(e.target.value);
                setTime(t);
                timeRef.current = t;
                requestRenderRef.current();
              }}
            />
            <span className="anim-val">{time.toFixed(2)}</span>
          </label>
          <div className="export">
            <button onClick={() => onDownload(exportFmt)} disabled={status.triangleCount === 0}>
              Export
            </button>
            <select
              aria-label="Export format"
              value={exportFmt}
              onChange={(e) => setExportFmt(e.target.value as "stl" | "off" | "obj")}
            >
              <option value="stl">STL</option>
              <option value="off">OFF</option>
              <option value="obj">OBJ</option>
            </select>
          </div>
        </div>
      </header>

      <div className="workspace">
        <div className="editor-col">
          <div className="tabs">
            {files.map((f, i) => (
              <div
                key={i}
                className={`tab ${i === active ? "active" : ""}`}
                onClick={() => switchTo(i)}
                onDoubleClick={() => renameFile(i)}
                title={i === 0 ? "main (rendered)" : "double-click to rename"}
              >
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
            ))}
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
