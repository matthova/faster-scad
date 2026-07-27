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

const DEFAULT_SOURCE = `// Quito playground — edits re-render live.
$fn = 48;

module bracket(w, d, h, t) {
  difference() {
    cube([w, d, h], center = true);
    cube([w - 2*t, d - 2*t, h + 1], center = true);
  }
}

difference() {
  union() {
    bracket(40, 30, 20, 3);
    // rounded boss
    translate([0, 0, 10])
      cylinder(h = 8, r1 = 9, r2 = 6);
  }
  // bolt pattern
  for (dx = [-15, 15], dy = [-10, 10])
    translate([dx, dy, -20])
      cylinder(h = 40, r = 2.5);
  // central bore
  cylinder(h = 60, r = 4, center = true);
}

echo("triangles will render live");
`;

interface Status {
  ok: boolean;
  message: string;
  triangleCount: number;
  volume: number;
  ms: number;
  echo: string;
}

export function App() {
  const editorHost = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewerRef = useRef<Viewer | null>(null);
  const engineRef = useRef<Engine | null>(null);
  const lastPositions = useRef<Float32Array>(new Float32Array(0));
  const debounceTimer = useRef<number | undefined>(undefined);

  const [status, setStatus] = useState<Status>({
    ok: true,
    message: "initializing…",
    triangleCount: 0,
    volume: 0,
    ms: 0,
    echo: "",
  });
  const [version, setVersion] = useState("");

  useEffect(() => {
    if (!canvasRef.current || !editorHost.current) return;

    const viewer = new Viewer(canvasRef.current);
    viewerRef.current = viewer;

    const engine = new Engine((r: RenderResponse) => onResult(r));
    engineRef.current = engine;

    const requestRender = (source: string) => {
      window.clearTimeout(debounceTimer.current);
      debounceTimer.current = window.setTimeout(() => {
        engine.render(source);
      }, 200);
    };

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
            if (u.docChanged) requestRender(u.state.doc.toString());
          }),
        ],
      }),
      parent: editorHost.current,
    });

    // initial render
    engine.render(DEFAULT_SOURCE);

    return () => {
      view.destroy();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function onResult(r: RenderResponse) {
    if (r.version) setVersion(r.version);
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
      });
    } else {
      setStatus((s) => ({ ...s, ok: false, message: r.error, ms: r.ms, echo: r.echo }));
    }
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
      </div>

      <footer className={`statusbar ${status.ok ? "ok" : "err"}`}>
        <span className="status-main">{status.message}</span>
        {status.ok && (
          <span className="status-meta">
            vol {status.volume.toFixed(2)} · {status.ms.toFixed(0)} ms
          </span>
        )}
        {status.echo && <span className="status-echo">{status.echo.replace(/\n/g, " | ")}</span>}
        <span className="status-version">{version && `engine ${version}`}</span>
      </footer>
    </div>
  );
}
