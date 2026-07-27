// Desktop engine: when running inside the Tauri shell, rendering goes over IPC
// to the NATIVE engine (C++ Manifold kernel) instead of the in-browser wasm
// worker — much faster, with include/use resolved from disk. Presents the same
// interface as `Engine` so `App` can use either transparently.
import type { RenderResponse } from "./engineWorker";

/** True when running inside the Tauri desktop shell. */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// Shape returned by the Rust `render` command (serde camelCase).
interface NativeResult {
  ok: boolean;
  error: string;
  echo: string;
  warnings: string;
  positions: number[];
  normals: number[];
  triangleCount: number;
  vertexCount: number;
  volume: number;
  area: number;
  is2D: boolean;
  params: string;
}

export class DesktopEngine {
  private seq = 0;
  /** Directory of the opened file, used for disk include/use resolution. */
  dir = ".";

  constructor(private onResult: (r: RenderResponse) => void) {}

  render(
    source: string,
    names: string[] = [],
    values: string[] = [],
    fileNames: string[] = [],
    fileContents: string[] = [],
  ) {
    const seq = ++this.seq;
    const t0 = performance.now();
    // Lazy import so the browser bundle never evaluates the Tauri API.
    import("@tauri-apps/api/core")
      .then(({ invoke }) =>
        invoke<NativeResult>("render", {
          source,
          dir: this.dir,
          paramNames: names,
          paramValues: values,
          fileNames,
          fileContents,
        }),
      )
      .then((res) => {
        if (seq !== this.seq) return; // superseded by a newer render
        this.onResult({
          seq,
          ok: res.ok,
          error: res.error,
          echo: res.echo,
          warnings: res.warnings,
          positions: new Float32Array(res.positions),
          normals: new Float32Array(res.normals),
          triangleCount: res.triangleCount,
          vertexCount: res.vertexCount,
          volume: res.volume,
          area: res.area,
          is2D: res.is2D,
          ms: performance.now() - t0,
          version: "native",
          params: res.params,
        });
      })
      .catch((e) => {
        if (seq !== this.seq) return;
        this.onResult({
          seq,
          ok: false,
          error: `engine error: ${String(e)}`,
          echo: "",
          warnings: "",
          positions: new Float32Array(0),
          normals: new Float32Array(0),
          triangleCount: 0,
          vertexCount: 0,
          volume: 0,
          area: 0,
          is2D: false,
          ms: performance.now() - t0,
          version: "native",
          params: `{"params":[]}`,
        });
      });
  }
}

/** A file opened from disk (native). */
export interface OpenedFile {
  path: string;
  name: string;
  dir: string;
  content: string;
}

/** Show a native open dialog and load the chosen `.scad` file (native only). */
export async function openScadFile(): Promise<OpenedFile | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const { invoke } = await import("@tauri-apps/api/core");
  const path = await open({
    multiple: false,
    filters: [{ name: "OpenSCAD", extensions: ["scad"] }],
  });
  if (!path || typeof path !== "string") return null;
  return invoke<OpenedFile>("open_file", { path });
}

/** Subscribe to external edits of the opened file. Returns an unlisten fn. */
export async function onFileChanged(
  cb: (payload: { path: string; content: string }) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<{ path: string; content: string }>("file-changed", (e) => cb(e.payload));
}

/** Native save via a Tauri save dialog + the `save_model` command. */
export async function saveModelNative(
  format: "stl" | "off" | "obj" | "3mf" | "amf" | "dxf" | "svg",
  source: string,
  names: string[],
  values: string[],
  fileNames: string[],
  fileContents: string[],
): Promise<void> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const { invoke } = await import("@tauri-apps/api/core");
  const path = await save({
    defaultPath: `quito.${format}`,
    filters: [{ name: format.toUpperCase(), extensions: [format] }],
  });
  if (!path) return; // cancelled
  await invoke("save_model", {
    path,
    format,
    source,
    dir: ".",
    paramNames: names,
    paramValues: values,
    fileNames,
    fileContents,
  });
}
