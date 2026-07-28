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
  diagnostics: string;
  previewPositions: number[];
  previewNormals: number[];
  groups: string;
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
          diagnostics: res.diagnostics,
          previewPositions: new Float32Array(res.previewPositions),
          previewNormals: new Float32Array(res.previewNormals),
          groups: res.groups,
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
          diagnostics: "[]",
          previewPositions: new Float32Array(0),
          previewNormals: new Float32Array(0),
          groups: "",
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

/** Load a `.scad` file by known path (open-with / double-click). */
export async function openScadPath(path: string): Promise<OpenedFile> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<OpenedFile>("open_file", { path });
}

/** A `.scad` path passed at launch (double-click), or null. Drain once on mount. */
export async function takePendingOpen(): Promise<string | null> {
  const { invoke } = await import("@tauri-apps/api/core");
  return (await invoke<string | null>("take_pending_open")) ?? null;
}

/** Write source text to a known disk path (⌘S on an already-saved tab). */
export async function saveSource(path: string, content: string): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("save_source", { path, content });
}

/** Show a Save dialog (default `.scad`) and write; returns the chosen path or null. */
export async function saveSourceAs(content: string, defaultName = "untitled.scad"): Promise<string | null> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const { invoke } = await import("@tauri-apps/api/core");
  const path = await save({
    defaultPath: defaultName,
    filters: [{ name: "OpenSCAD", extensions: ["scad"] }],
  });
  if (!path) return null; // cancelled
  await invoke("save_source", { path, content });
  return path;
}

/** Watch every project file with a disk path for external edits. */
export async function watchFiles(paths: string[]): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("watch_files", { paths });
}

/** Subscribe to `open-path` (warm open-with). Returns an unlisten fn. */
export async function onOpenPath(cb: (path: string) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("open-path", (e) => cb(e.payload));
}

/** Subscribe to native menu actions. Returns an unlisten fn. */
export async function onMenuAction(cb: (action: string) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("menu-action", (e) => cb(e.payload));
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
