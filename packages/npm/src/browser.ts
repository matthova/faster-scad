// Browser / bundler entry (the default `.` export for web and CDN consumers).
//
// The wasm module is fetched and instantiated on first use via `init()`. Call
// `ensureReady(wasmUrl?)` up front to control when — and from where — the
// `.wasm` loads (preload it, or point at a custom/CDN URL); otherwise the first
// `render()` initializes it, resolving `openrscad_bg.wasm` next to the glue module.
import init, {
  render_with_files as rawRenderWithFiles,
  export_2d as rawExport2d,
  parameters as rawParameters,
  version as rawVersion,
  clear_cache as rawClearCache,
} from "../pkg/web/openrscad.js";
import { makeApi, type RawEngine } from "./core.js";

let ready: Promise<void> | null = null;

/** Initialize the wasm module (idempotent — safe to call repeatedly). Pass a
 *  URL/Request/etc. to load the `.wasm` from a custom location; omit it to
 *  resolve `openrscad_bg.wasm` beside the glue module. */
export function ensureReady(wasmUrl?: string | URL | Request): Promise<void> {
  return (ready ??= init(wasmUrl ? { module_or_path: wasmUrl } : undefined).then(() => undefined));
}

const engine = {
  render_with_files: rawRenderWithFiles,
  export_2d: rawExport2d,
  parameters: rawParameters,
  version: rawVersion,
  clear_cache: rawClearCache,
} as unknown as RawEngine;

const api = makeApi(engine, ensureReady);

export const render = api.render;
export const exportShape2D = api.exportShape2D;
export const parameters = api.parameters;
export const version = api.version;
export const clearCache = api.clearCache;

export type { RenderOutput, RenderOptions, Diagnostic } from "./core.js";
