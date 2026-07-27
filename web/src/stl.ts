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

export function downloadBlob(data: Uint8Array, filename: string) {
  const blob = new Blob([data as BlobPart], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
