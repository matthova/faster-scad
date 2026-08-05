// A factory for fully-populated `RenderResponse` objects. Synthetic results
// (a stopped/timed-out render) must travel the exact same `onResult` path as a
// real render, so they need every field present. Kept in its own module — with a
// type-only import of `RenderResponse` — so importing it never pulls in the
// engine worker's side-effecting module top-level (which calls `init()`).
import type { RenderResponse } from "./engineWorker";

/** A blank (empty-geometry) RenderResponse with `patch` applied on top. */
export function blankResponse(
  seq: number,
  patch: Partial<RenderResponse> = {},
): RenderResponse {
  return {
    seq,
    ok: false,
    error: "",
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
    ms: 0,
    version: "",
    params: `{"params":[]}`,
    diagnostics: "[]",
    previewPositions: new Float32Array(0),
    previewNormals: new Float32Array(0),
    groups: "",
    provenancePositions: new Float32Array(0),
    provenanceNormals: new Float32Array(0),
    provenance: "",
    viewport: "",
    ...patch,
  };
}
