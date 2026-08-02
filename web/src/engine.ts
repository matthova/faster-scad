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
import type { RenderRequest, RenderResponse } from "./engineWorker";
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
}

/** Render a 2D model to DXF/SVG text via a dedicated one-shot worker. */
export function export2dBrowser(req: Export2DRequest): Promise<string> {
  return new Promise((resolve, reject) => {
    const w = new Worker(new URL("./exportWorker.ts", import.meta.url), { type: "module" });
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

/** A pending render request: source, overrides, and extra files. */
interface Job {
  source: string;
  names: string[];
  values: string[];
  fileNames: string[];
  fileContents: string[];
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

  private spawn() {
    this.worker = new Worker(new URL("./engineWorker.ts", import.meta.url), {
      type: "module",
    });
    this.worker.onmessage = (e: MessageEvent<RenderResponse>) => {
      this.clearTimer();
      this.setBusy(false);
      this.onResult(e.data);
      if (this.pending !== null) {
        const job = this.pending;
        this.pending = null;
        this.render(job.source, job.names, job.values, job.fileNames, job.fileContents);
      }
    };
  }

  render(
    source: string,
    names: string[] = [],
    values: string[] = [],
    fileNames: string[] = [],
    fileContents: string[] = [],
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
    } satisfies RenderRequest);
  }

  /** Stop the in-flight render (user pressed Stop): terminate the worker, drop
   *  any queued job, and deliver a synthetic "stopped" result so the UI idles. */
  cancel() {
    if (!this.busy) return;
    this.abort("Render stopped.");
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
    this.worker.terminate();
    this.spawn();
    this.pending = null;
    this.seq += 1;
    this.setBusy(false);
    this.onResult(blankResponse(this.seq, { error }));
  }
}
