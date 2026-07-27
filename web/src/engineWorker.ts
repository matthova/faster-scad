/// <reference lib="webworker" />
// The engine worker: initializes the wasm module once, then renders on demand.
import init, { render_with_params, parameters, version } from "../engine/quito.js";

export interface RenderRequest {
  seq: number;
  source: string;
  /** Parameter override names, parallel to `values`. */
  names: string[];
  /** Override values as literal strings ("30", "true", "\"hi\"", "[1,2,3]"). */
  values: string[];
}

export interface RenderResponse {
  seq: number;
  ok: boolean;
  error: string;
  echo: string;
  warnings: string;
  positions: Float32Array;
  normals: Float32Array;
  triangleCount: number;
  vertexCount: number;
  volume: number;
  area: number;
  ms: number;
  version: string;
  /** Customizer schema JSON (`{"params":[…]}`) for the current source. */
  params: string;
}

const ready = init();

self.onmessage = async (e: MessageEvent<RenderRequest>) => {
  const { seq, source, names, values } = e.data;
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
    res = render_with_params(source, names, values);
  } catch (err) {
    postMessage({
      seq,
      ok: false,
      error: `engine panic: ${String(err)}`,
      echo: "",
      warnings: "",
      positions: new Float32Array(0),
      normals: new Float32Array(0),
      triangleCount: 0,
      vertexCount: 0,
      volume: 0,
      area: 0,
      ms: performance.now() - t0,
      version: "",
      params,
    } satisfies RenderResponse);
    return;
  }

  const positions = res.positions;
  const normals = res.normals;
  const msg: RenderResponse = {
    seq,
    ok: res.ok,
    error: res.error,
    echo: res.echo,
    warnings: res.warnings,
    positions,
    normals,
    triangleCount: res.triangle_count,
    vertexCount: res.vertex_count,
    volume: res.volume,
    area: res.area,
    ms: performance.now() - t0,
    version: version(),
    params,
  };
  res.free();
  (self as unknown as Worker).postMessage(msg, [positions.buffer, normals.buffer]);
};
