// Manages the engine worker with real cancellation: if a render is requested
// while one is in flight, the worker is terminated and respawned (warm respawn),
// giving true cancellation of the superseded render (serial-wasm strategy from
// the plan). Results are delivered latest-wins.
import type { RenderRequest, RenderResponse } from "./engineWorker";

export class Engine {
  private worker!: Worker;
  private busy = false;
  private seq = 0;
  private pending: string | null = null;

  constructor(private onResult: (r: RenderResponse) => void) {
    this.spawn();
  }

  private spawn() {
    this.worker = new Worker(new URL("./engineWorker.ts", import.meta.url), {
      type: "module",
    });
    this.worker.onmessage = (e: MessageEvent<RenderResponse>) => {
      this.busy = false;
      this.onResult(e.data);
      if (this.pending !== null) {
        const src = this.pending;
        this.pending = null;
        this.render(src);
      }
    };
  }

  render(source: string) {
    if (this.busy) {
      // Cancel the in-flight render by terminating; respawn fresh.
      this.worker.terminate();
      this.spawn();
    }
    this.busy = true;
    this.seq += 1;
    this.worker.postMessage({ seq: this.seq, source } satisfies RenderRequest);
  }
}
