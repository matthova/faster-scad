/// <reference lib="webworker" />
// The engine worker: initializes the wasm module once, then renders on demand.
import init, { render_with_files, parameters, version } from "../engine/quito.js";

export interface RenderRequest {
  seq: number;
  source: string;
  /** Parameter override names, parallel to `values`. */
  names: string[];
  /** Override values as literal strings ("30", "true", "\"hi\"", "[1,2,3]"). */
  values: string[];
  /** Extra file names for include/use resolution, parallel to `fileContents`. */
  fileNames: string[];
  fileContents: string[];
}

export interface RenderResponse {
  seq: number;
  ok: boolean;
  error: string;
  /** Recoverable geometry errors (degraded render): newline-joined messages for
   *  CSG ops that failed and were replaced by a fallback mesh (e.g. non-manifold
   *  operands). Non-empty means a mesh is shown but is geometrically wrong
   *  somewhere; distinct from `error` (a hard failure with no mesh). */
  geomErrors: string;
  echo: string;
  warnings: string;
  positions: Float32Array;
  normals: Float32Array;
  triangleCount: number;
  vertexCount: number;
  volume: number;
  area: number;
  /** True when the model is a 2D object (exportable to DXF/SVG). */
  is2D: boolean;
  ms: number;
  version: string;
  /** Customizer schema JSON (`{"params":[…]}`) for the current source. */
  params: string;
  /** Structured diagnostics JSON (`[{severity,message,start,end}]`) for inline
   *  editor squiggles; start/end are byte offsets into the source, or -1. */
  diagnostics: string;
  /** Preview color channel — a concatenated triangle soup and a JSON array of
   *  per-group `{start,count,color,mode}`. Empty when the model uses no
   *  `color`/`#`/`%` (the viewer then uses `positions`). */
  previewPositions: Float32Array;
  previewNormals: Float32Array;
  groups: string;
  /** Provenance channel for editor↔preview linking (3D models): a per-statement
   *  triangle soup plus a JSON array of `{start,count,span}` (span = `[s,e]` byte
   *  offsets into the source, or `null`). Empty for 2D/empty models. */
  provenancePositions: Float32Array;
  provenanceNormals: Float32Array;
  provenance: string;
  /** `$vp*` viewport variables as JSON, or "" when the source has no `$vp`. */
  viewport: string;
  /** True when this is a synthetic result from a *stopped* render — a watchdog
   *  timeout or a user Stop — rather than a real engine result. The render never
   *  actually finished, so the app leaves the crash-recovery sentinel in place
   *  (see project.ts) instead of clearing it, and the next launch recovers
   *  instead of re-triggering the freeze. Absent on genuine results. */
  stopped?: boolean;
}

const ready = init();

self.onmessage = async (e: MessageEvent<RenderRequest>) => {
  const { seq, source, names, values, fileNames, fileContents } = e.data;
  await ready;

  const t0 = performance.now();
  let params = `{"params":[]}`;
  try {
    params = parameters(source);
  } catch {
    // keep the empty schema
  }

  let res;
  try {
    res = render_with_files(source, names, values, fileNames, fileContents);
  } catch (err) {
    // A wasm call-stack overflow (V8's limit) surfaces as a RangeError; give a
    // human-readable hint instead of the raw engine message.
    const raw = String(err);
    const error = /call stack|RangeError/i.test(raw)
      ? "recursion too deep (a non-tail-recursive function nested too far). Try an accumulator/tail-recursive form or reduce the depth."
      : `engine error: ${raw}`;
    postMessage({
      seq,
      ok: false,
      error,
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
      version: "",
      params,
      diagnostics: "[]",
      previewPositions: new Float32Array(0),
      previewNormals: new Float32Array(0),
      groups: "",
      provenancePositions: new Float32Array(0),
      provenanceNormals: new Float32Array(0),
      provenance: "",
      viewport: "",
    } satisfies RenderResponse);
    return;
  }

  const positions = res.positions;
  const normals = res.normals;
  const previewPositions = res.preview_positions;
  const previewNormals = res.preview_normals;
  const provenancePositions = res.provenance_positions;
  const provenanceNormals = res.provenance_normals;
  const msg: RenderResponse = {
    seq,
    ok: res.ok,
    error: res.error,
    geomErrors: res.geom_errors,
    echo: res.echo,
    warnings: res.warnings,
    positions,
    normals,
    triangleCount: res.triangle_count,
    vertexCount: res.vertex_count,
    volume: res.volume,
    area: res.area,
    is2D: res.is_2d,
    ms: performance.now() - t0,
    version: version(),
    params,
    diagnostics: res.diagnostics,
    previewPositions,
    previewNormals,
    groups: res.groups,
    provenancePositions,
    provenanceNormals,
    provenance: res.provenance,
    viewport: res.viewport,
  };
  res.free();
  (self as unknown as Worker).postMessage(msg, [
    positions.buffer,
    normals.buffer,
    previewPositions.buffer,
    previewNormals.buffer,
    provenancePositions.buffer,
    provenanceNormals.buffer,
  ]);
};
