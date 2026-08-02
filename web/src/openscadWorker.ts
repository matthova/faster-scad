/// <reference lib="webworker" />
// The OpenSCAD engine worker: an alternate to `engineWorker.ts` that renders
// with *actual* OpenSCAD (the official WebAssembly build) instead of Quito, so
// users can switch engines from the toolbar. It speaks the exact same
// RenderRequest → RenderResponse contract, so `Engine` (see engine.ts) drives it
// with the same latest-wins scheduling, watchdog, and terminate-on-cancel.
//
// OpenSCAD's wasm is vendored under `public/openscad/` and loaded lazily via a
// runtime dynamic import (its URL is passed in each request as `openscadUrl`, so
// it resolves correctly under any deploy base). A fresh module instance is built
// per render: the upstream loader is designed for repeated instantiation, and a
// clean instance sidesteps any main()-re-entry / stale-FS issues. Cancellation
// still works because `Engine` terminates the whole worker.
//
// The "Fast" toggle maps to OpenSCAD's F5 preview: the model is exported as
// colored OFF (with `$preview=true`) so `color(...)` shows, mirroring the
// OpenSCAD web playground. Fast off exports a plain binary STL (F6-style, single
// color). Geometry is exact (Manifold) in both modes; only color differs.
//
// Deliberate limitations of this path (documented, not bugs): no customizer
// schema (params is empty), no editor↔preview provenance channel, and 3D meshes
// only (2D models export empty). This is the official nightly WebAssembly build
// (OpenSCAD 2025.03.25), rendered with the Manifold backend (`--backend=manifold`)
// — the same build the OpenSCAD web playground ships.
import type { RenderRequest, RenderResponse } from "./engineWorker";
import { blankResponse } from "./renderResponse";
import { buildColoredFromOff, measure, parseBinaryStl } from "./openscadGeometry";

const OPENSCAD_VERSION = "OpenSCAD 2025.03.25 (Manifold)";

// Minimal shape of the upstream Emscripten module we rely on.
interface OpenSCADFS {
  mkdir(path: string): void;
  writeFile(path: string, data: string | ArrayBufferView): void;
  readFile(path: string, opts: { encoding: "binary" }): Uint8Array;
  unlink(path: string): void;
}
interface OpenSCADModule {
  callMain(args: string[]): number;
  FS: OpenSCADFS;
}
type OpenSCADFactory = (opts: Record<string, unknown>) => Promise<OpenSCADModule>;

/** Ensure every parent directory of `path` exists in the Emscripten FS. */
function mkdirp(FS: OpenSCADFS, path: string) {
  const parts = path.split("/").filter(Boolean);
  let cur = "";
  for (const p of parts) {
    cur += "/" + p;
    try {
      FS.mkdir(cur);
    } catch {
      // already exists — fine
    }
  }
}

self.onmessage = async (e: MessageEvent<RenderRequest>) => {
  const { seq, source, names, values, fileNames, fileContents, openscadUrl, preview } = e.data;
  const t0 = performance.now();

  const fail = (error: string) => {
    (self as unknown as Worker).postMessage(
      blankResponse(seq, {
        error,
        version: OPENSCAD_VERSION,
        ms: performance.now() - t0,
      }),
    );
  };

  if (!openscadUrl) {
    fail("engine error: missing OpenSCAD asset URL");
    return;
  }

  const out: string[] = [];
  const err: string[] = [];
  let instance: OpenSCADModule;
  try {
    const mod = (await import(/* @vite-ignore */ openscadUrl)) as {
      default: OpenSCADFactory;
    };
    instance = await mod.default({
      noInitialRun: true,
      print: (s: string) => out.push(s),
      printErr: (s: string) => err.push(s),
    });
  } catch (loadErr) {
    fail(`engine error: failed to load OpenSCAD wasm (${String(loadErr)})`);
    return;
  }

  const FS = instance.FS;
  try {
    // Materialize the include/use closure at absolute paths so relative includes
    // in the main file (`include <BOSL2/std.scad>`) resolve against `/`.
    for (let i = 0; i < fileNames.length; i++) {
      const path = "/" + fileNames[i].replace(/^\/+/, "");
      const slash = path.lastIndexOf("/");
      if (slash > 0) mkdirp(FS, path.slice(0, slash));
      FS.writeFile(path, fileContents[i]);
    }
    // Preview (Fast) mode mirrors OpenSCAD's F5: set `$preview` so `$preview`-
    // aware scripts render their preview form, and export colored OFF so
    // `color(...)` shows. Exact (Fast off) mode exports a plain binary STL — the
    // final F6-style render. Geometry is exact either way; only color differs.
    FS.writeFile("/main.scad", preview ? `$preview=true;\n${source}` : source);

    const outPath = preview ? "/out.off" : "/out.stl";
    const args: string[] = [
      "/main.scad",
      "-o",
      outPath,
      "--backend=manifold",
      `--export-format=${preview ? "off" : "binstl"}`,
    ];
    // Customizer / camera overrides (values are already OpenSCAD literals). One
    // `-Dname=value` arg each, matching the OpenSCAD playground's invocation.
    for (let i = 0; i < names.length; i++) {
      args.push(`-D${names[i]}=${values[i]}`);
    }

    const code = instance.callMain(args);

    const allLines = [...out, ...err];
    const echo = allLines.filter((l) => l.startsWith("ECHO:")).join("\n");
    const warnings = allLines.filter((l) => /WARNING:/.test(l)).join("\n");
    const errorLines = allLines.filter((l) => /ERROR:/.test(l));

    let data: Uint8Array | null = null;
    try {
      data = FS.readFile(outPath, { encoding: "binary" });
    } catch {
      data = null;
    }

    if (code !== 0 || !data || data.byteLength === 0) {
      const detail = errorLines.length
        ? errorLines.join("\n")
        : /not a 3D object/.test(err.join("\n"))
          ? "OpenSCAD produced no 3D geometry. The OpenSCAD engine renders 3D models only; 2D shapes (e.g. bare square/circle) aren't previewed — extrude them, or switch to the Quito engine."
          : `OpenSCAD exited with code ${code}.`;
      (self as unknown as Worker).postMessage(
        blankResponse(seq, {
          error: detail,
          echo,
          warnings,
          version: OPENSCAD_VERSION,
          ms: performance.now() - t0,
        }),
      );
      return;
    }

    let positions: Float32Array;
    let normals: Float32Array;
    let previewPositions: Float32Array = new Float32Array(0);
    let previewNormals: Float32Array = new Float32Array(0);
    let groups = "";
    if (preview) {
      // Colored F5-style preview: parse the colored OFF into a grouped soup and
      // feed the viewer's colored channel. `positions`/`normals` carry the same
      // geometry (distinct buffers) so stats and exports work unchanged.
      const built = buildColoredFromOff(new TextDecoder().decode(data));
      previewPositions = built.positions;
      previewNormals = built.normals;
      groups = JSON.stringify(built.groups);
      positions = new Float32Array(built.positions);
      normals = new Float32Array(built.normals);
    } else {
      // Copy into a standalone ArrayBuffer (the FS view may alias wasm memory).
      const parsed = parseBinaryStl(data.slice().buffer);
      positions = parsed.positions;
      normals = parsed.normals;
    }
    const { volume, area } = measure(positions);

    const msg: RenderResponse = {
      seq,
      ok: true,
      error: "",
      geomErrors: "",
      echo,
      warnings,
      positions,
      normals,
      triangleCount: positions.length / 9,
      vertexCount: positions.length / 3,
      volume,
      area,
      is2D: false,
      ms: performance.now() - t0,
      version: OPENSCAD_VERSION,
      params: `{"params":[]}`,
      diagnostics: "[]",
      previewPositions,
      previewNormals,
      groups,
      provenancePositions: new Float32Array(0),
      provenanceNormals: new Float32Array(0),
      provenance: "",
      viewport: "",
      // Geometry is exact (F6) either way — only color/preview-mode differs — so
      // exports use it directly (no re-render), unlike Quito's fast preview.
      preview: false,
    };
    (self as unknown as Worker).postMessage(
      msg,
      preview
        ? [positions.buffer, normals.buffer, previewPositions.buffer, previewNormals.buffer]
        : [positions.buffer, normals.buffer],
    );
  } catch (renderErr) {
    fail(`engine error: ${String(renderErr)}`);
  }
};
