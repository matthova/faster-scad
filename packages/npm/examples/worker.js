// A module Web Worker that runs the Quito engine off the main thread — the
// recommended way to use it in a browser app (rendering a large model can take
// tens of milliseconds and shouldn't block the UI).
//
// Spawn it from your app with a bundler that supports module workers, e.g. Vite:
//
//   const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
//   worker.onmessage = (e) => { /* e.data.result is a RenderOutput */ };
//   worker.postMessage({ seq: 1, source: "cube(10);" });
//
import { render } from "quito-engine";

self.onmessage = async (e) => {
  const { seq, source, params, files } = e.data;
  const result = await render(source, { params, files });
  // Transfer the (largest) mesh buffers to hand them over without a copy.
  self.postMessage({ seq, result }, [result.positions.buffer, result.normals.buffer]);
};
