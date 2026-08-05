// Desktop engine: when running inside the Tauri shell, rendering goes over IPC
// to the NATIVE engine (C++ Manifold kernel) instead of the in-browser wasm
// worker — much faster, with include/use resolved from disk. Presents the same
// interface as `Engine` so `App` can use either transparently.
import type { RenderResponse } from "./engineWorker";
import {
  OpenscadEngine,
  RENDER_TIMEOUT_MS,
  type EngineOptions,
} from "./engine";
import { blankResponse } from "./renderResponse";
import { assembleOpenscadResponse } from "./openscadGeometry";

/** True when running inside the Tauri desktop shell. */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Open a URL in the user's default browser. In the desktop shell a plain
 * `target="_blank"` link would try to navigate the webview, so route it through
 * the Tauri opener plugin (system browser). In the browser build, just open a
 * new tab.
 */
export async function openExternal(url: string): Promise<void> {
  if (!isTauri()) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}

// Shape returned by the Rust `render` command (serde camelCase).
interface NativeResult {
  ok: boolean;
  error: string;
  // Recoverable geometry errors (degraded render); "" when geometry is exact.
  // Optional so an older shell that predates the channel still deserializes.
  geomErrors?: string;
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
  // Provenance channel for editor↔preview linking (picking + highlight). The
  // native backend populates these for any model with geometry; kept optional so
  // an older shell that predates the channel still deserializes cleanly.
  provenancePositions?: number[];
  provenanceNormals?: number[];
  provenance?: string;
  viewport: string;
}

export class DesktopEngine {
  private seq = 0;
  /** Directory of the opened file, used for disk include/use resolution. */
  dir = ".";
  private busy = false;
  private timer: number | undefined;
  private readonly timeoutMs: number;
  /** Real native engine version, from the `engine_version` command. Falls back
   *  to "native" until the (one-shot, cached) query resolves. */
  private version = "native";

  constructor(
    private onResult: (r: RenderResponse) => void,
    private opts: EngineOptions = {},
  ) {
    this.timeoutMs = opts.timeoutMs ?? RENDER_TIMEOUT_MS;
    // Prefetch the engine version so results report it instead of "native".
    import("@tauri-apps/api/core")
      .then(({ invoke }) => invoke<string>("engine_version"))
      .then((v) => {
        if (v) this.version = v;
      })
      .catch(() => {}); // keep the "native" fallback
  }

  private setBusy(busy: boolean) {
    if (this.busy === busy) return;
    this.busy = busy;
    this.opts.onBusyChange?.(busy);
  }

  private clearTimer() {
    if (this.timer !== undefined) {
      window.clearTimeout(this.timer);
      this.timer = undefined;
    }
  }

  render(
    source: string,
    names: string[] = [],
    values: string[] = [],
    fileNames: string[] = [],
    fileContents: string[] = [],
    preview = false,
  ) {
    const seq = ++this.seq;
    this.setBusy(true);
    this.clearTimer();
    if (this.timeoutMs > 0) {
      this.timer = window.setTimeout(() => this.onTimeout(seq), this.timeoutMs);
    }
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
          preview,
        }),
      )
      .then((res) => {
        if (seq !== this.seq) return; // superseded by a newer render
        this.clearTimer();
        this.setBusy(false);
        this.onResult({
          seq,
          ok: res.ok,
          error: res.error,
          geomErrors: res.geomErrors ?? "",
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
          version: this.version,
          params: res.params,
          diagnostics: res.diagnostics,
          previewPositions: new Float32Array(res.previewPositions),
          previewNormals: new Float32Array(res.previewNormals),
          groups: res.groups,
          provenancePositions: new Float32Array(res.provenancePositions ?? []),
          provenanceNormals: new Float32Array(res.provenanceNormals ?? []),
          provenance: res.provenance ?? "",
          viewport: res.viewport,
          preview,
        });
      })
      .catch((e) => {
        if (seq !== this.seq) return;
        this.clearTimer();
        this.setBusy(false);
        this.onResult({
          seq,
          ok: false,
          error: `engine error: ${String(e)}`,
          geomErrors: "",
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
          version: this.version,
          params: `{"params":[]}`,
          diagnostics: "[]",
          previewPositions: new Float32Array(0),
          previewNormals: new Float32Array(0),
          groups: "",
          provenancePositions: new Float32Array(0),
          provenanceNormals: new Float32Array(0),
          provenance: "",
          viewport: "",
        });
      });
  }

  /** Stop waiting on the in-flight render and idle the UI. Bumping `seq` makes
   *  the native result a no-op when it eventually lands. NOTE: the native engine
   *  runs out-of-process and can't be killed from JS, so it keeps computing in
   *  the background; this frees the UI, not the CPU. */
  cancel() {
    if (!this.busy) return;
    this.abort(
      "Render stopped. (The native engine may still be finishing in the background.)",
    );
  }

  /** Tear down (parity with `Engine.dispose`). The native render runs
   *  out-of-process, so bump `seq` to ignore any in-flight result and idle. */
  dispose() {
    this.clearTimer();
    this.seq += 1;
    this.setBusy(false);
  }

  private onTimeout(seq: number) {
    if (seq !== this.seq) return;
    this.timer = undefined;
    this.abort(
      `Render stopped after ${Math.round(this.timeoutMs / 1000)}s — the model may be too ` +
        `complex. Simplify it, then press Render. (The native engine may still be finishing ` +
        `in the background.)`,
    );
  }

  private abort(error: string) {
    this.clearTimer();
    this.seq += 1; // ignore the in-flight native result when it lands
    this.setBusy(false);
    this.onResult(
      blankResponse(this.seq, { error, version: this.version, stopped: true }),
    );
  }
}

/** Shape returned by the Rust `render_openscad` command (serde camelCase). */
interface NativeOpenscadRun {
  /** False when no local OpenSCAD binary was found — trigger the wasm fallback. */
  available: boolean;
  ok: boolean;
  error: string;
  echo: string;
  warnings: string;
  version: string;
  preview: boolean;
  /** Exported bytes (OFF when `preview`, else binary STL) as a byte array. */
  data: number[];
}

/**
 * Desktop OpenSCAD engine: renders with a *locally-installed* OpenSCAD binary
 * (its fast Manifold backend) over IPC, then parses the exported bytes with the
 * same helpers the in-browser wasm OpenSCAD engine uses, so both emit identical
 * geometry/preview/stats channels. Presents the same interface as
 * `DesktopEngine`/`Engine` so `App` drives it transparently.
 *
 * If no local binary is found (`available: false`), it transparently and
 * permanently falls back to the vendored wasm `OpenscadEngine` — still real
 * OpenSCAD, just slower — so the toggle works whether or not OpenSCAD is
 * installed. Set `QUITO_OPENSCAD=/path/to/openscad` to point at a specific build.
 */
export class DesktopOpenscadEngine {
  private seq = 0;
  /** Directory of the opened file, added to OPENSCADPATH for disk include/use. */
  dir = ".";
  private busy = false;
  private timer: number | undefined;
  private readonly timeoutMs: number;
  /** Lazily-created wasm fallback; once set, all renders delegate to it. */
  private fallback: OpenscadEngine | null = null;

  constructor(
    private onResult: (r: RenderResponse) => void,
    private opts: EngineOptions = {},
  ) {
    this.timeoutMs = opts.timeoutMs ?? RENDER_TIMEOUT_MS;
  }

  private setBusy(busy: boolean) {
    if (this.busy === busy) return;
    this.busy = busy;
    this.opts.onBusyChange?.(busy);
  }

  private clearTimer() {
    if (this.timer !== undefined) {
      window.clearTimeout(this.timer);
      this.timer = undefined;
    }
  }

  render(
    source: string,
    names: string[] = [],
    values: string[] = [],
    fileNames: string[] = [],
    fileContents: string[] = [],
    preview = false,
  ) {
    if (this.fallback) {
      this.fallback.render(
        source,
        names,
        values,
        fileNames,
        fileContents,
        preview,
      );
      return;
    }
    const seq = ++this.seq;
    this.setBusy(true);
    this.clearTimer();
    if (this.timeoutMs > 0) {
      this.timer = window.setTimeout(() => this.onTimeout(seq), this.timeoutMs);
    }
    const t0 = performance.now();
    import("@tauri-apps/api/core")
      .then(({ invoke }) =>
        invoke<NativeOpenscadRun>("render_openscad", {
          source,
          dir: this.dir,
          paramNames: names,
          paramValues: values,
          fileNames,
          fileContents,
          preview,
        }),
      )
      .then((run) => {
        if (seq !== this.seq) return; // superseded by a newer render
        if (!run.available) {
          // No local OpenSCAD binary — switch to the vendored wasm engine for
          // this and all future renders (it manages its own busy/watchdog).
          this.clearTimer();
          this.fallback = new OpenscadEngine(this.onResult, this.opts);
          this.fallback.render(
            source,
            names,
            values,
            fileNames,
            fileContents,
            preview,
          );
          return;
        }
        this.clearTimer();
        this.setBusy(false);
        const data = run.data.length ? Uint8Array.from(run.data) : null;
        const { message } = assembleOpenscadResponse(seq, {
          ok: run.ok,
          data,
          preview: run.preview,
          echo: run.echo,
          warnings: run.warnings,
          error: run.error,
          version: run.version,
          ms: performance.now() - t0,
        });
        this.onResult(message);
      })
      .catch((e) => {
        if (seq !== this.seq) return;
        this.clearTimer();
        this.setBusy(false);
        this.onResult(
          blankResponse(seq, {
            error: `OpenSCAD engine error: ${String(e)}`,
            version: "OpenSCAD (local)",
          }),
        );
      });
  }

  /** Stop waiting on the in-flight render and idle the UI. Like `DesktopEngine`,
   *  the native process runs out-of-process and can't be killed from JS. */
  cancel() {
    if (this.fallback) {
      this.fallback.cancel();
      return;
    }
    if (!this.busy) return;
    this.abort(
      "Render stopped. (OpenSCAD may still be finishing in the background.)",
    );
  }

  dispose() {
    this.fallback?.dispose();
    this.clearTimer();
    this.seq += 1;
    this.setBusy(false);
  }

  private onTimeout(seq: number) {
    if (seq !== this.seq) return;
    this.timer = undefined;
    this.abort(
      `Render stopped after ${Math.round(this.timeoutMs / 1000)}s — the model may be too ` +
        `complex. Simplify it, then press Render. (OpenSCAD may still be finishing in the ` +
        `background.)`,
    );
  }

  private abort(error: string) {
    this.clearTimer();
    this.seq += 1; // ignore the in-flight native result when it lands
    this.setBusy(false);
    this.onResult(
      blankResponse(this.seq, {
        error,
        version: "OpenSCAD (local)",
        stopped: true,
      }),
    );
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
export async function saveSourceAs(
  content: string,
  defaultName = "untitled.scad",
): Promise<string | null> {
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

/** Save binary bytes (e.g. a captured PNG) via a native save dialog. */
export async function saveImageNative(
  bytes: Uint8Array,
  defaultName = "quito.png",
): Promise<void> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const { invoke } = await import("@tauri-apps/api/core");
  const path = await save({
    defaultPath: defaultName,
    filters: [{ name: "PNG", extensions: ["png"] }],
  });
  if (!path) return; // cancelled
  await invoke("save_bytes", { path, bytes: Array.from(bytes) });
}

/** Watch every project file with a disk path for external edits. */
export async function watchFiles(paths: string[]): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("watch_files", { paths });
}

/** Subscribe to `open-path` (warm open-with). Returns an unlisten fn. */
export async function onOpenPath(
  cb: (path: string) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("open-path", (e) => cb(e.payload));
}

/** Subscribe to native menu actions. Returns an unlisten fn. */
export async function onMenuAction(
  cb: (action: string) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("menu-action", (e) => cb(e.payload));
}

/** Subscribe to external edits of the opened file. Returns an unlisten fn. */
export async function onFileChanged(
  cb: (payload: { path: string; content: string }) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<{ path: string; content: string }>("file-changed", (e) =>
    cb(e.payload),
  );
}

/** Save already-built export bytes via a native save dialog. Used when a wasm
 *  engine (e.g. OpenSCAD) produced the geometry on desktop, so the native
 *  re-render path (`save_model`) — which would use the native Quito engine —
 *  doesn't apply and we write the bytes we already have. */
export async function saveBytesNative(
  bytes: Uint8Array,
  format: string,
): Promise<void> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const { invoke } = await import("@tauri-apps/api/core");
  const path = await save({
    defaultPath: `quito.${format}`,
    filters: [{ name: format.toUpperCase(), extensions: [format] }],
  });
  if (!path) return; // cancelled
  await invoke("save_bytes", { path, bytes: Array.from(bytes) });
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
