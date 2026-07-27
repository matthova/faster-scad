import { useEffect, useRef, useState } from "react";
import { EditorView, keymap } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { openscad } from "./lang/openscad";
import { Viewer } from "./viewer";
import { Engine } from "./engine";
import type { RenderResponse } from "./engineWorker";
import { buildBinarySTL, downloadBlob } from "./stl";
import { CustomizerPanel } from "./CustomizerPanel";
import { parseSchema, toLiteral, type Param, type ParamValue } from "./customizer";

const DEFAULT_SOURCE = `// Quito playground — edits re-render live.
// Drag the parameters on the right (they come from these annotated variables).
$fn = 48;

/* [Bracket] */
// outer width
width = 40;   // [20:80]
// outer depth
depth = 30;   // [20:80]
height = 20;  // [10:40]
// wall thickness
wall = 3;     // [1:0.5:6]

/* [Boss] */
boss = true;
boss_h = 8;   // [0:20]

module bracket(w, d, h, t) {
  difference() {
    cube([w, d, h], center = true);
    cube([w - 2*t, d - 2*t, h + 1], center = true);
  }
}

difference() {
  union() {
    bracket(width, depth, height, wall);
    if (boss)
      translate([0, 0, height/2])
        cylinder(h = boss_h, r1 = 9, r2 = 6);
  }
  for (dx = [-width/2 + 8, width/2 - 8], dy = [-depth/2 + 8, depth/2 - 8])
    translate([dx, dy, -height])
      cylinder(h = 2*height, r = 2.5);
  cylinder(h = 3*height, r = 4, center = true);
}

echo("bounding box", width, depth, height);
`;

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
  const engineRef = useRef<Engine | null>(null);
  const lastPositions = useRef<Float32Array>(new Float32Array(0));
  const debounceTimer = useRef<number | undefined>(undefined);

  // Customizer state. Refs mirror the state so imperative render calls (from
  // slider drags and editor edits) never see a stale closure.
  const sourceRef = useRef(DEFAULT_SOURCE);
  const overridesRef = useRef<Record<string, ParamValue>>({});
  const paramsJsonRef = useRef("");
  const requestRenderRef = useRef<() => void>(() => {});

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
  const [schema, setSchema] = useState<Param[]>([]);
  const [overrides, setOverrides] = useState<Record<string, ParamValue>>({});

  useEffect(() => {
    if (!canvasRef.current || !editorHost.current) return;

    const viewer = new Viewer(canvasRef.current);
    viewerRef.current = viewer;

    const engine = new Engine((r: RenderResponse) => onResult(r));
    engineRef.current = engine;

    const renderNow = () => {
      const ov = overridesRef.current;
      const names = Object.keys(ov);
      const values = names.map((n) => toLiteral(ov[n]));
      engine.render(sourceRef.current, names, values);
    };
    const requestRender = () => {
      window.clearTimeout(debounceTimer.current);
      debounceTimer.current = window.setTimeout(renderNow, 150);
    };
    requestRenderRef.current = requestRender;

    const view = new EditorView({
      state: EditorState.create({
        doc: DEFAULT_SOURCE,
        extensions: [
          basicSetup,
          keymap.of([indentWithTab]),
          openscad(),
          EditorView.theme({
            "&": { height: "100%", fontSize: "13px" },
            ".cm-scroller": { fontFamily: "ui-monospace, Menlo, monospace" },
          }),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) {
              sourceRef.current = u.state.doc.toString();
              requestRender();
            }
          }),
        ],
      }),
      parent: editorHost.current,
    });

    renderNow(); // initial render

    return () => {
      view.destroy();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function onResult(r: RenderResponse) {
    if (r.version) setVersion(r.version);

    // Update the parameter schema when it changed. Preserve override values for
    // params that still exist (by name), drop the rest.
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
      setConsoleOpen(true); // surface failures immediately
    }
  }

  // Console lines with severity, newest section first.
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
    requestRenderRef.current();
  }

  function resetOverrides() {
    overridesRef.current = {};
    setOverrides({});
    requestRenderRef.current();
  }

  function onDownload() {
    if (lastPositions.current.length === 0) return;
    downloadBlob(buildBinarySTL(lastPositions.current), "quito.stl");
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          Quito <span className="tag">playground</span>
        </div>
        <div className="actions">
          <button onClick={() => viewerRef.current?.resetView()}>Reset view</button>
          <button onClick={onDownload} disabled={status.triangleCount === 0}>
            Download STL
          </button>
        </div>
      </header>

      <div className="workspace">
        <div className="editor" ref={editorHost} />
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
