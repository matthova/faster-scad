// Manages the engine worker with real cancellation: if a render is requested
// while one is in flight, the worker is terminated and respawned (warm respawn),
// giving true cancellation of the superseded render (serial-wasm strategy from
// the plan). Results are delivered latest-wins.
//
// Two escape hatches keep a too-heavy model from spinning the worker forever
// with no way out (the "death spiral"): a watchdog auto-stops any render still
// running after `timeoutMs`, and `cancel()` lets the UI stop one on demand. Both
// terminate the worker (the only way to interrupt a synchronous wasm call) and
// deliver a synthetic error result so the app returns to an idle, usable state.
import type {
  EnginePhase,
  EnginePhaseMessage,
  RenderRequest,
  RenderResponse,
} from "./engineWorker";
import type { Export2DRequest, Export2DResponse } from "./exportWorker";
import { blankResponse } from "./renderResponse";

/** A render still running after this many ms is auto-stopped by the watchdog. */
export const RENDER_TIMEOUT_MS = 20_000;

export interface EngineOptions {
  /** Notified whenever the busy (render-in-flight) state flips, so the UI can
   *  show a Stop affordance and arm crash-recovery. */
  onBusyChange?: (busy: boolean) => void;
  /** Auto-stop a render still running after this many ms (0 disables). */
  timeoutMs?: number;
  /** Notified when a worker is downloading a large engine asset (OpenSCAD wasm)
   *  before it can render, so the UI can show a downloading state. The watchdog
   *  is paused for the duration; see `onPhase`. */
  onDownloadChange?: (downloading: boolean) => void;
}

/** Render a 2D model to DXF/SVG text via a dedicated one-shot worker. */
export function export2dBrowser(req: Export2DRequest): Promise<string> {
  return new Promise((resolve, reject) => {
    const w = new Worker(new URL("./exportWorker.ts", import.meta.url), {
      type: "module",
    });
    w.onmessage = (e: MessageEvent<Export2DResponse>) => {
      w.terminate();
      if (e.data.error) reject(new Error(e.data.error));
      else resolve(e.data.data);
    };
    w.onerror = (e) => {
      w.terminate();
      reject(new Error(e.message || "export worker error"));
    };
    w.postMessage(req);
  });
}

/** Positions from a one-shot **exact** (watertight) render, for export when the
 *  live viewer is showing a fast preview. Spawns a throwaway engine worker so it
 *  never disturbs the interactive render loop. */
export function renderMeshExactBrowser(job: {
  source: string;
  names: string[];
  values: string[];
  fileNames: string[];
  fileContents: string[];
}): Promise<Float32Array> {
  return new Promise((resolve, reject) => {
    const w = new Worker(new URL("./engineWorker.ts", import.meta.url), {
      type: "module",
    });
    w.onmessage = (e: MessageEvent<RenderResponse>) => {
      w.terminate();
      if (!e.data.ok) reject(new Error(e.data.error || "render failed"));
      else resolve(e.data.positions);
    };
    w.onerror = (e) => {
      w.terminate();
      reject(new Error(e.message || "engine worker error"));
    };
    w.postMessage({ seq: 0, preview: false, ...job } satisfies RenderRequest);
  });
}

/** A pending render request: source, overrides, and extra files. */
interface Job {
  source: string;
  names: string[];
  values: string[];
  fileNames: string[];
  fileContents: string[];
  preview: boolean;
}

export class Engine {
  private worker!: Worker;
  private busy = false;
  private seq = 0;
  private pending: Job | null = null;
  private timer: number | undefined;
  private readonly timeoutMs: number;

  constructor(
    private onResult: (r: RenderResponse) => void,
    private opts: EngineOptions = {},
  ) {
    this.timeoutMs = opts.timeoutMs ?? RENDER_TIMEOUT_MS;
    this.spawn();
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

  /** Construct the backing worker. Overridden by `OpenscadEngine` to spawn the
   *  OpenSCAD worker instead of the OpenRSCAD one. Kept as a literal `new Worker(new
   *  URL(...))` per call site so Vite can bundle each worker. */
  protected createWorker(): Worker {
    return new Worker(new URL("./engineWorker.ts", import.meta.url), {
      type: "module",
    });
  }

  /** Extra fields merged into every posted request. Overridden by
   *  `OpenscadEngine` to inject the wasm loader URL. */
  protected requestExtra(): Partial<RenderRequest> {
    return {};
  }

  private spawn() {
    this.worker = this.createWorker();
    this.worker.onmessage = (
      e: MessageEvent<RenderResponse | EnginePhaseMessage>,
    ) => {
      const msg = e.data;
      if ("phase" in msg) {
        this.onPhase(msg.phase);
        return;
      }
      this.clearTimer();
      this.opts.onDownloadChange?.(false);
      this.setBusy(false);
      this.onResult(msg);
      if (this.pending !== null) {
        const job = this.pending;
        this.pending = null;
        this.render(
          job.source,
          job.names,
          job.values,
          job.fileNames,
          job.fileContents,
          job.preview,
        );
      }
    };
  }

  render(
    source: string,
    names: string[] = [],
    values: string[] = [],
    fileNames: string[] = [],
    fileContents: string[] = [],
    preview = false,
  ) {
    if (this.busy) {
      // Cancel the in-flight render by terminating; respawn fresh.
      this.clearTimer();
      this.worker.terminate();
      this.spawn();
    }
    this.setBusy(true);
    this.seq += 1;
    if (this.timeoutMs > 0) {
      this.timer = window.setTimeout(() => this.onTimeout(), this.timeoutMs);
    }
    this.worker.postMessage({
      seq: this.seq,
      source,
      names,
      values,
      fileNames,
      fileContents,
      preview,
      ...this.requestExtra(),
    } satisfies RenderRequest);
  }

  /** Permanently tear down the worker (used when swapping to another engine).
   *  Unlike `cancel()`, no synthetic result is delivered — the caller is
   *  replacing this engine wholesale. */
  dispose() {
    this.clearTimer();
    this.opts.onDownloadChange?.(false);
    this.worker.terminate();
    this.pending = null;
    this.setBusy(false);
  }

  /** Stop the in-flight render (user pressed Stop): terminate the worker, drop
   *  any queued job, and deliver a synthetic "stopped" result so the UI idles. */
  cancel() {
    if (!this.busy) return;
    this.abort("Render stopped.");
  }

  /** Progress signal from a worker that must download a large asset before it
   *  can render (the OpenSCAD wasm engine). The download must not count against
   *  the render watchdog — otherwise a slow connection aborts with a misleading
   *  "model too complex" message — so pause the timer while `"loading"` and
   *  (re)arm it only when the render proper begins (`"rendering"`). */
  private onPhase(phase: EnginePhase) {
    if (phase === "loading") {
      this.clearTimer();
      this.opts.onDownloadChange?.(true);
    } else {
      this.opts.onDownloadChange?.(false);
      if (this.timeoutMs > 0) {
        this.timer = window.setTimeout(() => this.onTimeout(), this.timeoutMs);
      }
    }
  }

  private onTimeout() {
    this.timer = undefined;
    this.abort(
      `Render stopped after ${Math.round(this.timeoutMs / 1000)}s — the model may be ` +
        `too complex. Reduce $fn or simplify it, then press Render.`,
    );
  }

  private abort(error: string) {
    this.clearTimer();
    this.opts.onDownloadChange?.(false);
    this.worker.terminate();
    this.spawn();
    this.pending = null;
    this.seq += 1;
    this.setBusy(false);
    this.onResult(blankResponse(this.seq, { error, stopped: true }));
  }
}

/** Renders with the vendored OpenSCAD wasm instead of OpenRSCAD. Inherits all of
 *  `Engine`'s scheduling/watchdog/cancellation — only the worker and the
 *  injected loader URL differ. See openscadWorker.ts for the contract and its
 *  documented limitations. */
export class OpenscadEngine extends Engine {
  // The vendored loader lives in `public/openscad/`; resolve it against the
  // document base so it works under any deploy path (e.g. GitHub Pages subdir).
  private readonly url = new URL("openscad/openscad.js", document.baseURI).href;

  protected createWorker(): Worker {
    return new Worker(new URL("./openscadWorker.ts", import.meta.url), {
      type: "module",
    });
  }

  protected requestExtra(): Partial<RenderRequest> {
    return { openscadUrl: this.url };
  }
}
