// Environment-agnostic facade over the raw wasm-bindgen exports.
//
// The generated `render*` functions return a wasm-owned `RenderResult` whose
// getters COPY their data into fresh JS values (each getter clones), then the
// object must be `.free()`d or it leaks wasm linear memory on every render. This
// wrapper reads the fields callers actually want and always frees in a
// `finally`, so consumers never touch `.free()` and never leak. The browser and
// node entry points supply the concrete bindings and an `ensureReady` hook.

/** A structured diagnostic. `start`/`end` are UTF-8 byte offsets into the
 *  source (or -1 when unknown) — note JS strings are UTF-16, so map offsets if
 *  you place them in an editor. */
export interface Diagnostic {
  severity: "error" | "warning";
  message: string;
  start: number;
  end: number;
}

/** A parsed render. Mesh data is a non-indexed triangle soup: `positions` and
 *  `normals` both hold 9 f32 per triangle (flat, per-face normals), and are
 *  independent copies you own (safe to transfer to another worker/thread). */
export interface RenderOutput {
  ok: boolean;
  /** Error message, or "" when `ok`. */
  error: string;
  positions: Float32Array;
  normals: Float32Array;
  /** True for 2D models (exportable to DXF/SVG via `exportShape2D`). */
  is2d: boolean;
  triangleCount: number;
  vertexCount: number;
  volume: number;
  area: number;
  /** Newline-joined `ECHO:` output. */
  echo: string;
  /** Newline-joined warnings. */
  warnings: string;
  diagnostics: Diagnostic[];
  /** Per-color preview channel (a concatenated triangle soup + a JSON array of
   *  `{start,count,color,mode}`). Empty unless the model uses `color`/`#`/`%`
   *  (the viewer then uses `positions`). */
  preview: { positions: Float32Array; normals: Float32Array; groups: string };
  /** Editor↔preview provenance channel (a per-leaf triangle soup + a JSON array
   *  of `{start,count,spans}` byte-offset stacks). Empty for models with no
   *  geometry. */
  provenance: { positions: Float32Array; normals: Float32Array; groups: string };
  /** `$vp*` viewport variables as JSON, or "" when the source has no `$vp`. */
  viewport: string;
}

export interface RenderOptions {
  /** Customizer overrides. Values are OpenSCAD literals as strings — numbers and
   *  booleans are stringified for you, but strings must be quoted (`'"hi"'`) and
   *  vectors bracketed (`"[1,2,3]"`), matching the language. */
  params?: Record<string, string | number | boolean>;
  /** In-memory files for `include`/`use` resolution, keyed by path:
   *  `{ "lib.scad": "function f() = 1;" }`. */
  files?: Record<string, string>;
}

/** The subset of the wasm `RenderResult` this facade reads. */
interface RawResult {
  ok: boolean;
  error: string;
  positions: Float32Array;
  normals: Float32Array;
  is_2d: boolean;
  triangle_count: number;
  vertex_count: number;
  volume: number;
  area: number;
  echo: string;
  warnings: string;
  diagnostics: string;
  preview_positions: Float32Array;
  preview_normals: Float32Array;
  groups: string;
  provenance_positions: Float32Array;
  provenance_normals: Float32Array;
  provenance: string;
  viewport: string;
  free(): void;
}

/** The generated wasm-bindgen exports this facade wraps. */
export interface RawEngine {
  render_with_files(
    source: string,
    names: string[],
    values: string[],
    fileNames: string[],
    fileContents: string[],
  ): RawResult;
  export_2d(
    source: string,
    names: string[],
    values: string[],
    fileNames: string[],
    fileContents: string[],
    format: string,
  ): string;
  parameters(source: string): string;
  version(): string;
  clear_cache(): void;
}

function toLiteral(v: string | number | boolean): string {
  return typeof v === "string" ? v : String(v);
}

/** Build the public API from the raw engine bindings and an init hook. */
export function makeApi(engine: RawEngine, ensureReady: () => Promise<void>) {
  function split(opts: RenderOptions) {
    const paramEntries = Object.entries(opts.params ?? {});
    const fileEntries = Object.entries(opts.files ?? {});
    return {
      names: paramEntries.map(([k]) => k),
      values: paramEntries.map(([, v]) => toLiteral(v)),
      fileNames: fileEntries.map(([k]) => k),
      fileContents: fileEntries.map(([, v]) => v),
    };
  }

  /** Parse `.scad` source and render it to mesh + diagnostics. */
  async function render(source: string, opts: RenderOptions = {}): Promise<RenderOutput> {
    await ensureReady();
    const { names, values, fileNames, fileContents } = split(opts);
    const r = engine.render_with_files(source, names, values, fileNames, fileContents);
    try {
      return {
        ok: r.ok,
        error: r.error,
        positions: r.positions,
        normals: r.normals,
        is2d: r.is_2d,
        triangleCount: r.triangle_count,
        vertexCount: r.vertex_count,
        volume: r.volume,
        area: r.area,
        echo: r.echo,
        warnings: r.warnings,
        diagnostics: JSON.parse(r.diagnostics || "[]") as Diagnostic[],
        preview: {
          positions: r.preview_positions,
          normals: r.preview_normals,
          groups: r.groups,
        },
        provenance: {
          positions: r.provenance_positions,
          normals: r.provenance_normals,
          groups: r.provenance,
        },
        viewport: r.viewport,
      };
    } finally {
      r.free();
    }
  }

  /** Render a 2D model to DXF or SVG text (empty if the model isn't 2D). */
  async function exportShape2D(
    source: string,
    format: "dxf" | "svg",
    opts: RenderOptions = {},
  ): Promise<string> {
    await ensureReady();
    const { names, values, fileNames, fileContents } = split(opts);
    return engine.export_2d(source, names, values, fileNames, fileContents, format);
  }

  /** The customizer parameter schema for a source string, as JSON
   *  (`{"params":[…]}`). */
  async function parameters(source: string): Promise<string> {
    await ensureReady();
    return engine.parameters(source);
  }

  /** The engine version string (matches the npm package version). */
  async function version(): Promise<string> {
    await ensureReady();
    return engine.version();
  }

  /** Drop the persistent geometry cache (e.g. when loading a new document). */
  async function clearCache(): Promise<void> {
    await ensureReady();
    engine.clear_cache();
  }

  return { render, exportShape2D, parameters, version, clearCache };
}
