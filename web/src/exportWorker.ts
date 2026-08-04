/// <reference lib="webworker" />
// A one-shot worker that renders a 2D model and serializes it to DXF or SVG.
// Kept separate from the render worker (which terminates/respawns for
// cancellation) so a user-initiated export never races the live render loop.
import init, { export_2d } from "../engine/quito.js";

export interface Export2DRequest {
  source: string;
  names: string[];
  values: string[];
  fileNames: string[];
  fileContents: string[];
  format: "dxf" | "svg";
}

export interface Export2DResponse {
  data: string;
  error: string;
}

const ready = init();

self.onmessage = async (e: MessageEvent<Export2DRequest>) => {
  const { source, names, values, fileNames, fileContents, format } = e.data;
  await ready;
  let data = "";
  let error = "";
  try {
    data = export_2d(source, names, values, fileNames, fileContents, format);
    if (!data) error = "export produced no geometry (is the model 2D?)";
  } catch (err) {
    error = String(err);
  }
  (self as unknown as Worker).postMessage({
    data,
    error,
  } satisfies Export2DResponse);
};
