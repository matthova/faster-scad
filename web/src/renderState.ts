// The pure half of "a render landed". `reduce` folds a RenderResponse into the
// nine display-state fields with no side effects and no React — the imperative
// half (mesh upload, crash sentinel, editor squiggles, camera) lives in
// App.tsx's `applyRenderEffects`. Keeping this pure makes the render outcome
// unit-testable across every branch (ok / error / stopped / geom-errors /
// preview / 2D / schema-change / multi-color) without a DOM, and it is the
// rule §7 spells out: nothing reachable from onResult may read a `useState`
// value — here `prev` is threaded in explicitly instead.
//
// Do NOT import React (or anything that touches the DOM) into this file; the
// node-env test asserts the reducer runs without a browser.
import { parseSchema, type Param, type ParamValue } from "./customizer";
import type { PreviewGroup } from "./viewer";
import type { RenderResponse } from "./engineWorker";

export type ExportFmt = "stl" | "off" | "obj" | "3mf" | "amf" | "dxf" | "svg";

// 3D solids export to mesh formats, 2D profiles to vector formats.
export const FORMATS_3D: ExportFmt[] = ["stl", "off", "obj", "3mf", "amf"];
export const FORMATS_2D: ExportFmt[] = ["dxf", "svg"];

/** A structured diagnostic from the engine (byte offsets into the main source). */
export interface EngineDiag {
  severity: "error" | "warning";
  message: string;
  start: number; // UTF-8 byte offset, or -1 when unknown
  end: number;
}

export interface Status {
  ok: boolean;
  message: string;
  triangleCount: number;
  vertexCount: number;
  /** Total surface area (3D) or enclosed area (2D). */
  area: number;
  volume: number;
  ms: number;
  echo: string;
  warnings: string;
  error: string;
  /** Recoverable geometry errors (degraded render): a mesh is shown but a CSG op
   *  failed and was replaced by a fallback. Empty when geometry is exact. */
  geomErrors: string;
  /** The shown mesh came from the fast, non-watertight preview path, so `volume`
   *  is approximate (it counts skipped-union interior walls). */
  preview: boolean;
}

/** The nine fields a completed render replaces. Consolidated into one state so
 *  the reducer can read `prev` from inside the `[]`-deps mount effect (a
 *  functional `setState` sees only one slice's previous value). */
export interface RenderState {
  status: Status;
  schema: Param[];
  overrides: Record<string, ParamValue>;
  is2D: boolean;
  exportFmt: ExportFmt;
  version: string;
  diagCounts: { errors: number; warnings: number };
  /** Monotonic; surfaced as `data-render-rev` — the e2e "a render landed" signal. */
  renderRev: number;
  /** Snapshot of the source that produced the shown mesh (for span-based UI that
   *  must not read the live, possibly-newer editor buffer). Updated only on a
   *  successful render. */
  renderedSource: string;
}

export const INITIAL_RENDER_STATE: RenderState = {
  status: {
    ok: true,
    message: "initializing…",
    triangleCount: 0,
    vertexCount: 0,
    area: 0,
    volume: 0,
    ms: 0,
    echo: "",
    warnings: "",
    error: "",
    geomErrors: "",
    preview: false,
  },
  schema: [],
  overrides: {},
  is2D: false,
  exportFmt: "stl",
  version: "",
  diagCounts: { errors: 0, warnings: 0 },
  renderRev: 0,
  renderedSource: "",
};

/** Non-`(prev, r)` inputs the reducer needs, all plain values (refs read by the
 *  caller) so the reducer stays pure. */
export interface ReduceCtx {
  /** The user has manually chosen an export format (so stop auto-tracking it). */
  userPickedFmt: boolean;
  /** The main source that produced this render, snapshotted at request time. */
  renderedSource: string;
  /** `r.params` differs from the last-seen schema JSON (caller owns the cache),
   *  so re-parse the schema and drop overrides no longer in it. */
  paramsChanged: boolean;
}

/** Parse the structured-diagnostics channel; tolerant of a malformed payload. */
export function parseDiagnostics(json: string): EngineDiag[] {
  try {
    return JSON.parse(json || "[]") as EngineDiag[];
  } catch {
    return [];
  }
}

/** Colored-preview groups; tolerant of a malformed payload. */
function parseGroups(json: string): PreviewGroup[] {
  if (!json) return [];
  try {
    return JSON.parse(json) as PreviewGroup[];
  } catch {
    return [];
  }
}

/** A model is "multi-color" for export when its exportable (non-`%` background)
 *  color groups use more than one distinct color. STL drops color; 3MF keeps it
 *  as separate objects — so multi-color models default to 3MF. */
export function distinctExportColors(groups: PreviewGroup[]): number {
  const seen = new Set<string>();
  for (const g of groups) {
    if (g.mode === "background") continue;
    seen.add(g.color.join(","));
  }
  return seen.size;
}

/** Drop overrides whose parameter is no longer in `schema`; preserve the input
 *  reference when nothing changed so React can bail out of a re-render. */
export function keepOverrides(
  overrides: Record<string, ParamValue>,
  schema: Param[],
): Record<string, ParamValue> {
  const kept: Record<string, ParamValue> = {};
  for (const [k, v] of Object.entries(overrides)) {
    if (schema.some((p) => p.name === k)) kept[k] = v;
  }
  return Object.keys(kept).length === Object.keys(overrides).length
    ? overrides
    : kept;
}

function nextFmt(
  prev: ExportFmt,
  is2D: boolean,
  multiColor: boolean,
  userPicked: boolean,
): ExportFmt {
  if (is2D) return FORMATS_2D.includes(prev) ? prev : "dxf";
  // Until the user picks, default multi-color models to 3MF (preserves colors)
  // and everything else to STL.
  if (!userPicked) return multiColor ? "3mf" : "stl";
  return FORMATS_3D.includes(prev) ? prev : "stl";
}

/** Fold a render response into the display state. Pure: same inputs → same
 *  output, no side effects. `r.stopped` (watchdog/Stop) lands here as an
 *  ordinary `ok:false` result — the stopped-specific behaviour is all in the
 *  effects half. */
export function reduce(
  prev: RenderState,
  r: RenderResponse,
  ctx: ReduceCtx,
): RenderState {
  const diags = parseDiagnostics(r.diagnostics);
  const diagCounts = {
    errors: diags.filter((d) => d.severity === "error").length,
    warnings: diags.filter((d) => d.severity === "warning").length,
  };
  const version = r.version || prev.version;
  const renderRev = prev.renderRev + 1;

  let schema = prev.schema;
  let overrides = prev.overrides;
  if (ctx.paramsChanged) {
    schema = parseSchema(r.params);
    overrides = keepOverrides(prev.overrides, schema);
  }

  if (r.ok) {
    const groups = parseGroups(r.groups);
    const multiColor = distinctExportColors(groups) > 1;
    return {
      status: {
        ok: true,
        message: r.geomErrors
          ? `${r.triangleCount.toLocaleString()} triangles · geometry errors`
          : `${r.triangleCount.toLocaleString()} triangles`,
        triangleCount: r.triangleCount,
        vertexCount: r.vertexCount,
        area: r.area,
        volume: r.volume,
        ms: r.ms,
        echo: r.echo,
        warnings: r.warnings,
        error: "",
        geomErrors: r.geomErrors,
        preview: r.preview ?? false,
      },
      schema,
      overrides,
      is2D: r.is2D,
      exportFmt: nextFmt(prev.exportFmt, r.is2D, multiColor, ctx.userPickedFmt),
      version,
      diagCounts,
      renderRev,
      renderedSource: ctx.renderedSource,
    };
  }

  // Failure (hard error or watchdog/Stop): keep the last good mesh's stats
  // (triangleCount/volume/…), dimensionality, format, and rendered source; only
  // the error-facing fields change.
  return {
    status: {
      ...prev.status,
      ok: false,
      message: r.error,
      ms: r.ms,
      echo: r.echo,
      warnings: r.warnings,
      error: r.error,
      geomErrors: r.geomErrors,
    },
    schema,
    overrides,
    is2D: prev.is2D,
    exportFmt: prev.exportFmt,
    version,
    diagCounts,
    renderRev,
    renderedSource: prev.renderedSource,
  };
}
