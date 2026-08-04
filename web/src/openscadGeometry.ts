// Pure geometry helpers for the OpenSCAD engine: parse OpenSCAD's export output
// (binary STL, colored OFF) into the viewer's mesh channels, measure a triangle
// soup, and assemble a RenderResponse from a completed OpenSCAD run. Shared by
// both OpenSCAD paths — the in-browser wasm worker (openscadWorker.ts) and the
// native desktop engine that shells out to a local OpenSCAD (desktopEngine.ts) —
// so they emit identical geometry, preview, and stats channels. Kept free of
// worker side-effects so it's unit-testable in plain Node.
import type { RenderResponse } from "./engineWorker";
import { blankResponse } from "./renderResponse";

/** A per-face colored group in the viewer's colored-mesh channel (mirrors
 *  `PreviewGroup` in viewer.ts). `start`/`count` are vertex indices into the
 *  soup (a triangle is 3 vertices). */
export interface ColorGroup {
  start: number;
  count: number;
  color: [number, number, number, number];
  mode: "solid";
}

// OpenSCAD's default preview color (the familiar gold), for faces the OFF export
// leaves uncolored. Matches OpenSCAD's own `0xf9d72c`.
export const DEFAULT_COLOR: [number, number, number, number] = [
  0xf9 / 255,
  0xd7 / 255,
  0x2c / 255,
  1,
];

/** Parse a binary STL blob into flat vertex + per-face-normal arrays (three's
 *  non-indexed layout). Throws if the byte length doesn't match the header. */
export function parseBinaryStl(buf: ArrayBuffer): {
  positions: Float32Array<ArrayBuffer>;
  normals: Float32Array<ArrayBuffer>;
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

/** Build a colored triangle soup (grouped by color) from an OpenSCAD-exported
 *  OFF file. With the Manifold backend, OFF faces carry per-face RGB(A) colors
 *  (0–255) reflecting `color(...)` — the same channel OpenSCAD's F5 preview
 *  shows. Faces without an explicit color fall back to the default gold. Faces
 *  may be polygons, so they're fan-triangulated. */
export function buildColoredFromOff(text: string): {
  positions: Float32Array<ArrayBuffer>;
  normals: Float32Array<ArrayBuffer>;
  groups: ColorGroup[];
} {
  const lines = text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0 && !l.startsWith("#"));
  if (lines.length === 0) throw new Error("empty OFF file");

  // The header "OFF" may be alone on its line or followed by the counts.
  let idx: number;
  let counts: string;
  const first = lines[0];
  if (first === "OFF") {
    counts = lines[1];
    idx = 2;
  } else if (/^OFF\s+/.test(first)) {
    counts = first.slice(3).trim();
    idx = 1;
  } else {
    throw new Error("not an OFF file");
  }
  const [numVerts, numFaces] = counts.split(/\s+/).map(Number);
  if (!Number.isFinite(numVerts) || !Number.isFinite(numFaces)) {
    throw new Error("invalid OFF counts");
  }

  const verts = new Float64Array(numVerts * 3);
  for (let i = 0; i < numVerts; i++) {
    const p = lines[idx + i].split(/\s+/);
    verts[i * 3] = parseFloat(p[0]);
    verts[i * 3 + 1] = parseFloat(p[1]);
    verts[i * 3 + 2] = parseFloat(p[2]);
  }
  idx += numVerts;

  // Collect triangles grouped by color (dedup colors into stable groups).
  const byColor = new Map<
    string,
    { color: [number, number, number, number]; tris: number[][] }
  >();
  for (let i = 0; i < numFaces; i++) {
    const p = lines[idx + i].split(/\s+/).map(Number);
    const n = p[0];
    const vi = p.slice(1, 1 + n);
    // Color columns (if any) follow the vertex indices: 3 (RGB) or 4 (RGBA).
    let color: [number, number, number, number] = DEFAULT_COLOR;
    if (p.length >= n + 4) {
      const c = p.slice(n + 1);
      color = [
        c[0] / 255,
        c[1] / 255,
        c[2] / 255,
        c.length >= 4 ? c[3] / 255 : 1,
      ];
    }
    const key = color.join(",");
    let g = byColor.get(key);
    if (!g) {
      g = { color, tris: [] };
      byColor.set(key, g);
    }
    for (let j = 1; j < vi.length - 1; j++)
      g.tris.push([vi[0], vi[j], vi[j + 1]]);
  }

  let totalTris = 0;
  for (const g of byColor.values()) totalTris += g.tris.length;
  const positions = new Float32Array(totalTris * 9);
  const normals = new Float32Array(totalTris * 9);
  const groups: ColorGroup[] = [];
  let f = 0; // float write cursor
  let v = 0; // vertex write cursor
  for (const g of byColor.values()) {
    const start = v;
    for (const [a, b, c] of g.tris) {
      const ax = verts[a * 3],
        ay = verts[a * 3 + 1],
        az = verts[a * 3 + 2];
      const bx = verts[b * 3],
        by = verts[b * 3 + 1],
        bz = verts[b * 3 + 2];
      const cx = verts[c * 3],
        cy = verts[c * 3 + 1],
        cz = verts[c * 3 + 2];
      // Flat (per-face) normal, shared by the triangle's three vertices.
      let nx = (by - ay) * (cz - az) - (bz - az) * (cy - ay);
      let ny = (bz - az) * (cx - ax) - (bx - ax) * (cz - az);
      let nz = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
      const len = Math.hypot(nx, ny, nz) || 1;
      nx /= len;
      ny /= len;
      nz /= len;
      positions[f] = ax;
      positions[f + 1] = ay;
      positions[f + 2] = az;
      positions[f + 3] = bx;
      positions[f + 4] = by;
      positions[f + 5] = bz;
      positions[f + 6] = cx;
      positions[f + 7] = cy;
      positions[f + 8] = cz;
      for (let k = 0; k < 3; k++) {
        normals[f + k * 3] = nx;
        normals[f + k * 3 + 1] = ny;
        normals[f + k * 3 + 2] = nz;
      }
      f += 9;
      v += 3;
    }
    groups.push({
      start,
      count: g.tris.length * 3,
      color: g.color,
      mode: "solid",
    });
  }
  return { positions, normals, groups };
}

/** A completed OpenSCAD run: the exported bytes plus captured logs. `preview`
 *  selects how `data` is parsed — OFF (F5-style colored) vs binary STL (F6). */
export interface OpenscadRun {
  ok: boolean;
  /** Exported bytes: colored OFF when `preview`, else binary STL. */
  data: Uint8Array | null;
  preview: boolean;
  echo: string;
  warnings: string;
  /** Failure detail when `!ok` (empty output, non-zero exit, 2D model, …). */
  error: string;
  /** Engine version string for the status bar (differs wasm vs local build). */
  version: string;
  ms: number;
}

/** Assemble a `RenderResponse` from a completed OpenSCAD run, plus the list of
 *  buffers to transfer (empty on failure). Geometry is exact (F6) either way —
 *  only color/preview-mode differs — so exports reuse `positions` directly. */
export function assembleOpenscadResponse(
  seq: number,
  run: OpenscadRun,
): { message: RenderResponse; transfer: ArrayBuffer[] } {
  const { ok, data, preview, echo, warnings, error, version, ms } = run;
  if (!ok || !data || data.byteLength === 0) {
    return {
      message: blankResponse(seq, { error, echo, warnings, version, ms }),
      transfer: [],
    };
  }

  let positions: Float32Array<ArrayBuffer>;
  let normals: Float32Array<ArrayBuffer>;
  let previewPositions: Float32Array<ArrayBuffer> = new Float32Array(0);
  let previewNormals: Float32Array<ArrayBuffer> = new Float32Array(0);
  let groups = "";
  if (preview) {
    // Colored F5-style preview: parse the colored OFF into a grouped soup for the
    // viewer's colored channel. `positions`/`normals` carry the same geometry
    // (distinct buffers) so stats and exports work unchanged.
    const built = buildColoredFromOff(new TextDecoder().decode(data));
    previewPositions = built.positions;
    previewNormals = built.normals;
    groups = JSON.stringify(built.groups);
    positions = new Float32Array(built.positions);
    normals = new Float32Array(built.normals);
  } else {
    // Copy into a standalone ArrayBuffer (the source view may alias wasm memory).
    const parsed = parseBinaryStl(data.slice().buffer);
    positions = parsed.positions;
    normals = parsed.normals;
  }
  const { volume, area } = measure(positions);

  const message: RenderResponse = {
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
    ms,
    version,
    params: `{"params":[]}`,
    diagnostics: "[]",
    previewPositions,
    previewNormals,
    groups,
    provenancePositions: new Float32Array(0),
    provenanceNormals: new Float32Array(0),
    provenance: "",
    viewport: "",
    // Geometry is exact (F6) either way, so exports use it directly (no re-render).
    preview: false,
  };
  const transfer = preview
    ? [
        positions.buffer,
        normals.buffer,
        previewPositions.buffer,
        previewNormals.buffer,
      ]
    : [positions.buffer, normals.buffer];
  return { message, transfer };
}

/** Volume (via the divergence theorem) and surface area of a triangle soup. */
export function measure(positions: Float32Array): {
  volume: number;
  area: number;
} {
  let vol = 0;
  let area = 0;
  for (let i = 0; i < positions.length; i += 9) {
    const ax = positions[i],
      ay = positions[i + 1],
      az = positions[i + 2];
    const bx = positions[i + 3],
      by = positions[i + 4],
      bz = positions[i + 5];
    const cx = positions[i + 6],
      cy = positions[i + 7],
      cz = positions[i + 8];
    // signed volume of the tetrahedron (origin, a, b, c)
    vol +=
      (ax * (by * cz - bz * cy) -
        ay * (bx * cz - bz * cx) +
        az * (bx * cy - by * cx)) /
      6;
    // triangle area = |(b-a) × (c-a)| / 2
    const ux = bx - ax,
      uy = by - ay,
      uz = bz - az;
    const vx = cx - ax,
      vy = cy - ay,
      vz = cz - az;
    const wx = uy * vz - uz * vy;
    const wy = uz * vx - ux * vz;
    const wz = ux * vy - uy * vx;
    area += Math.sqrt(wx * wx + wy * wy + wz * wz) / 2;
  }
  return { volume: Math.abs(vol), area };
}
