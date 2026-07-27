// Build a binary STL from a flat triangle-soup position array (9 floats/tri).
export function buildBinarySTL(positions: Float32Array): Uint8Array {
  const nTri = Math.floor(positions.length / 9);
  const buf = new ArrayBuffer(84 + nTri * 50);
  const dv = new DataView(buf);
  dv.setUint32(80, nTri, true);
  let off = 84;
  for (let i = 0; i < nTri; i++) {
    const b = i * 9;
    const ax = positions[b], ay = positions[b + 1], az = positions[b + 2];
    const bx = positions[b + 3], by = positions[b + 4], bz = positions[b + 5];
    const cx = positions[b + 6], cy = positions[b + 7], cz = positions[b + 8];
    // face normal
    const ux = bx - ax, uy = by - ay, uz = bz - az;
    const vx = cx - ax, vy = cy - ay, vz = cz - az;
    let nx = uy * vz - uz * vy;
    let ny = uz * vx - ux * vz;
    let nz = ux * vy - uy * vx;
    const len = Math.hypot(nx, ny, nz) || 1;
    nx /= len; ny /= len; nz /= len;
    dv.setFloat32(off, nx, true); dv.setFloat32(off + 4, ny, true); dv.setFloat32(off + 8, nz, true);
    off += 12;
    for (let k = 0; k < 9; k++) {
      dv.setFloat32(off, positions[b + k], true);
      off += 4;
    }
    off += 2; // attribute byte count
  }
  return new Uint8Array(buf);
}

// Weld a triangle soup (9 floats/tri) into a unique vertex list + index faces.
function weld(positions: Float32Array): { verts: number[]; faces: [number, number, number][] } {
  const map = new Map<string, number>();
  const verts: number[] = [];
  const faces: [number, number, number][] = [];
  const idxOf = (x: number, y: number, z: number): number => {
    const key = `${x},${y},${z}`;
    let i = map.get(key);
    if (i === undefined) {
      i = verts.length / 3;
      verts.push(x, y, z);
      map.set(key, i);
    }
    return i;
  };
  const nTri = Math.floor(positions.length / 9);
  for (let t = 0; t < nTri; t++) {
    const b = t * 9;
    faces.push([
      idxOf(positions[b], positions[b + 1], positions[b + 2]),
      idxOf(positions[b + 3], positions[b + 4], positions[b + 5]),
      idxOf(positions[b + 6], positions[b + 7], positions[b + 8]),
    ]);
  }
  return { verts, faces };
}

/** OFF (Object File Format): a welded vertex list followed by triangle faces. */
export function buildOFF(positions: Float32Array): Uint8Array {
  const { verts, faces } = weld(positions);
  const out: string[] = ["OFF", `${verts.length / 3} ${faces.length} 0`];
  for (let i = 0; i < verts.length; i += 3) out.push(`${verts[i]} ${verts[i + 1]} ${verts[i + 2]}`);
  for (const f of faces) out.push(`3 ${f[0]} ${f[1]} ${f[2]}`);
  return new TextEncoder().encode(out.join("\n") + "\n");
}

/** Wavefront OBJ: welded vertices (`v`) and 1-based triangle faces (`f`). */
export function buildOBJ(positions: Float32Array): Uint8Array {
  const { verts, faces } = weld(positions);
  const out: string[] = ["# exported by Quito"];
  for (let i = 0; i < verts.length; i += 3) out.push(`v ${verts[i]} ${verts[i + 1]} ${verts[i + 2]}`);
  for (const f of faces) out.push(`f ${f[0] + 1} ${f[1] + 1} ${f[2] + 1}`);
  return new TextEncoder().encode(out.join("\n") + "\n");
}

export function downloadBlob(data: Uint8Array, filename: string) {
  const blob = new Blob([data as BlobPart], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
