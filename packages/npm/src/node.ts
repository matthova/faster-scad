// Node entry (the `node` + `import` export condition).
//
// The nodejs-target glue instantiates the wasm synchronously on import, so there
// is no async init step: `ensureReady()` resolves immediately and exists only
// for API parity with the browser entry. (CommonJS `require()` consumers resolve
// to the raw `pkg/node/openrscad.js` glue directly — see the package `exports` map.)
import {
  render_with_files as rawRenderWithFiles,
  export_2d as rawExport2d,
  parameters as rawParameters,
  version as rawVersion,
  clear_cache as rawClearCache,
} from "../pkg/node/openrscad.js";
import { makeApi, type RawEngine } from "./core.js";

/** Resolves immediately — the wasm is already instantiated on import in Node. */
export function ensureReady(): Promise<void> {
  return Promise.resolve();
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
