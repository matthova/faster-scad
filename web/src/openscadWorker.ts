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
// Deliberate limitations of this path (documented, not bugs): no customizer
// schema (params is empty), no editor↔preview provenance/color channels, 3D
// meshes only (2D models export empty), and no fast-preview mode. This is the
// official nightly WebAssembly build (OpenSCAD 2025.03.25), rendered with the
// Manifold backend (`--backend=manifold`) — the same build the OpenSCAD web
// playground ships.
import type { RenderRequest, RenderResponse } from "./engineWorker";
import { blankResponse } from "./renderResponse";

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

/** Parse a binary STL blob into flat vertex + per-face-normal arrays (three's
 *  non-indexed layout). Throws if the byte length doesn't match the header. */
function parseBinaryStl(buf: ArrayBuffer): {
  positions: Float32Array;
  normals: Float32Array;
} {
  if (buf.byteLength < 84) throw new Error("STL too short");
  const dv = new DataView(buf);
  const n = dv.getUint32(80, true);
  if (buf.byteLength !== 84 + n * 50) {
    throw new Error("unexpected STL layout (not binary?)");
  }
  const positions = new Float32Array(n * 9);
  const normals = new Float32Array(n * 9);
  let off = 84;
  for (let i = 0; i < n; i++) {
    const nx = dv.getFloat32(off, true);
    const ny = dv.getFloat32(off + 4, true);
    const nz = dv.getFloat32(off + 8, true);
    off += 12;
    for (let v = 0; v < 3; v++) {
      const p = i * 9 + v * 3;
      positions[p] = dv.getFloat32(off, true);
      positions[p + 1] = dv.getFloat32(off + 4, true);
      positions[p + 2] = dv.getFloat32(off + 8, true);
      normals[p] = nx;
      normals[p + 1] = ny;
      normals[p + 2] = nz;
      off += 12;
    }
    off += 2; // attribute byte count
  }
  return { positions, normals };
}

/** Volume (via the divergence theorem) and surface area of a triangle soup. */
function measure(positions: Float32Array): { volume: number; area: number } {
  let vol = 0;
  let area = 0;
  for (let i = 0; i < positions.length; i += 9) {
    const ax = positions[i], ay = positions[i + 1], az = positions[i + 2];
    const bx = positions[i + 3], by = positions[i + 4], bz = positions[i + 5];
    const cx = positions[i + 6], cy = positions[i + 7], cz = positions[i + 8];
    // signed volume of the tetrahedron (origin, a, b, c)
    vol +=
      (ax * (by * cz - bz * cy) -
        ay * (bx * cz - bz * cx) +
        az * (bx * cy - by * cx)) /
      6;
    // triangle area = |(b-a) × (c-a)| / 2
    const ux = bx - ax, uy = by - ay, uz = bz - az;
    const vx = cx - ax, vy = cy - ay, vz = cz - az;
    const wx = uy * vz - uz * vy;
    const wy = uz * vx - ux * vz;
    const wz = ux * vy - uy * vx;
    area += Math.sqrt(wx * wx + wy * wy + wz * wz) / 2;
  }
  return { volume: Math.abs(vol), area };
}

self.onmessage = async (e: MessageEvent<RenderRequest>) => {
  const { seq, source, names, values, fileNames, fileContents, openscadUrl } = e.data;
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
    FS.writeFile("/main.scad", source);

    const args: string[] = [
      "/main.scad",
      "-o",
      "/out.stl",
      "--backend=manifold",
      "--export-format=binstl",
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

    let stl: Uint8Array | null = null;
    try {
      stl = FS.readFile("/out.stl", { encoding: "binary" });
    } catch {
      stl = null;
    }

    if (code !== 0 || !stl || stl.byteLength === 0) {
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

    // Copy into a standalone ArrayBuffer (the FS view may alias wasm memory).
    const buf = stl.slice().buffer;
    const { positions, normals } = parseBinaryStl(buf);
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
      previewPositions: new Float32Array(0),
      previewNormals: new Float32Array(0),
      groups: "",
      provenancePositions: new Float32Array(0),
      provenanceNormals: new Float32Array(0),
      provenance: "",
      viewport: "",
      preview: false,
    };
    (self as unknown as Worker).postMessage(msg, [positions.buffer, normals.buffer]);
  } catch (renderErr) {
    fail(`engine error: ${String(renderErr)}`);
  }
};
