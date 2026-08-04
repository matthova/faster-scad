// Build a binary STL from a flat triangle-soup position array (9 floats/tri).
export function buildBinarySTL(positions: Float32Array): Uint8Array {
  const nTri = Math.floor(positions.length / 9);
  const buf = new ArrayBuffer(84 + nTri * 50);
  const dv = new DataView(buf);
  dv.setUint32(80, nTri, true);
  let off = 84;
  for (let i = 0; i < nTri; i++) {
    const b = i * 9;
    const ax = positions[b],
      ay = positions[b + 1],
      az = positions[b + 2];
    const bx = positions[b + 3],
      by = positions[b + 4],
      bz = positions[b + 5];
    const cx = positions[b + 6],
      cy = positions[b + 7],
      cz = positions[b + 8];
    // face normal
    const ux = bx - ax,
      uy = by - ay,
      uz = bz - az;
    const vx = cx - ax,
      vy = cy - ay,
      vz = cz - az;
    let nx = uy * vz - uz * vy;
    let ny = uz * vx - ux * vz;
    let nz = ux * vy - uy * vx;
    const len = Math.hypot(nx, ny, nz) || 1;
    nx /= len;
    ny /= len;
    nz /= len;
    dv.setFloat32(off, nx, true);
    dv.setFloat32(off + 4, ny, true);
    dv.setFloat32(off + 8, nz, true);
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
function weld(positions: Float32Array): {
  verts: number[];
  faces: [number, number, number][];
} {
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
  for (let i = 0; i < verts.length; i += 3)
    out.push(`${verts[i]} ${verts[i + 1]} ${verts[i + 2]}`);
  for (const f of faces) out.push(`3 ${f[0]} ${f[1]} ${f[2]}`);
  return new TextEncoder().encode(out.join("\n") + "\n");
}

/** Wavefront OBJ: welded vertices (`v`) and 1-based triangle faces (`f`). */
export function buildOBJ(positions: Float32Array): Uint8Array {
  const { verts, faces } = weld(positions);
  const out: string[] = ["# exported by Quito"];
  for (let i = 0; i < verts.length; i += 3)
    out.push(`v ${verts[i]} ${verts[i + 1]} ${verts[i + 2]}`);
  for (const f of faces) out.push(`f ${f[0] + 1} ${f[1] + 1} ${f[2] + 1}`);
  return new TextEncoder().encode(out.join("\n") + "\n");
}

/** AMF (Additive Manufacturing Format): plain XML, welded indexed mesh. */
export function buildAMF(positions: Float32Array): Uint8Array {
  const { verts, faces } = weld(positions);
  const out: string[] = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<amf unit="millimeter">',
    ' <object id="0">',
    "  <mesh>",
    "   <vertices>",
  ];
  for (let i = 0; i < verts.length; i += 3)
    out.push(
      `   <vertex><coordinates><x>${verts[i]}</x><y>${verts[i + 1]}</y><z>${verts[i + 2]}</z></coordinates></vertex>`,
    );
  out.push("   </vertices>", "   <volume>");
  for (const f of faces)
    out.push(
      `    <triangle><v1>${f[0]}</v1><v2>${f[1]}</v2><v3>${f[2]}</v3></triangle>`,
    );
  out.push("   </volume>", "  </mesh>", " </object>", "</amf>", "");
  return new TextEncoder().encode(out.join("\n"));
}

/** The `3D/3dmodel.model` XML for a 3MF package (core spec). */
function threeMFModel(positions: Float32Array): string {
  const { verts, faces } = weld(positions);
  const out: string[] = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">',
    " <resources>",
    '  <object id="1" type="model">',
    "   <mesh>",
    "    <vertices>",
  ];
  for (let i = 0; i < verts.length; i += 3)
    out.push(
      `     <vertex x="${verts[i]}" y="${verts[i + 1]}" z="${verts[i + 2]}"/>`,
    );
  out.push("    </vertices>", "    <triangles>");
  for (const f of faces)
    out.push(`     <triangle v1="${f[0]}" v2="${f[1]}" v3="${f[2]}"/>`);
  out.push(
    "    </triangles>",
    "   </mesh>",
    "  </object>",
    " </resources>",
    " <build>",
    '  <item objectid="1"/>',
    " </build>",
    "</model>",
    "",
  );
  return out.join("\n");
}

const CT_XML =
  '<?xml version="1.0" encoding="UTF-8"?>\n<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">\n <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>\n <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>\n</Types>\n';
const RELS_XML =
  '<?xml version="1.0" encoding="UTF-8"?>\n<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">\n <Relationship Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel" Target="/3D/3dmodel.model" Id="rel0"/>\n</Relationships>\n';

/** 3MF: a ZIP of the model XML plus the OPC content-types and rels parts. */
export function build3MF(positions: Float32Array): Uint8Array {
  const enc = new TextEncoder();
  return storeZip([
    { name: "[Content_Types].xml", data: enc.encode(CT_XML) },
    { name: "_rels/.rels", data: enc.encode(RELS_XML) },
    { name: "3D/3dmodel.model", data: enc.encode(threeMFModel(positions)) },
  ]);
}

/** A colored preview group: a vertex range into the soup plus color + mode. */
export interface ColorGroup {
  start: number;
  count: number;
  color: [number, number, number, number];
  mode: string;
}

function colorHex(c: [number, number, number, number]): string {
  const b = (x: number) =>
    Math.round(Math.min(1, Math.max(0, x)) * 255)
      .toString(16)
      .padStart(2, "0")
      .toUpperCase();
  return `#${b(c[0])}${b(c[1])}${b(c[2])}${b(c[3])}`;
}

/** The `3D/3dmodel.model` XML for a colored 3MF: one object per group, each
 *  bound to a `<base displaycolor>`. `groups` index into the preview soup by
 *  vertex offset (`start`/`count`). */
function threeMFColoredModel(
  previewPositions: Float32Array,
  groups: ColorGroup[],
): string {
  const out: string[] = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">',
    " <resources>",
    '  <basematerials id="1">',
  ];
  groups.forEach((g, i) =>
    out.push(`   <base name="c${i}" displaycolor="${colorHex(g.color)}"/>`),
  );
  out.push("  </basematerials>");
  groups.forEach((g, i) => {
    const oid = i + 2;
    const slice = previewPositions.subarray(
      g.start * 3,
      (g.start + g.count) * 3,
    );
    const { verts, faces } = weld(slice);
    out.push(
      `  <object id="${oid}" type="model" pid="1" pindex="${i}">`,
      "   <mesh>",
      "    <vertices>",
    );
    for (let j = 0; j < verts.length; j += 3)
      out.push(
        `     <vertex x="${verts[j]}" y="${verts[j + 1]}" z="${verts[j + 2]}"/>`,
      );
    out.push("    </vertices>", "    <triangles>");
    for (const f of faces)
      out.push(`     <triangle v1="${f[0]}" v2="${f[1]}" v3="${f[2]}"/>`);
    out.push("    </triangles>", "   </mesh>", "  </object>");
  });
  out.push(" </resources>", " <build>");
  groups.forEach((_, i) => out.push(`  <item objectid="${i + 2}"/>`));
  out.push(" </build>", "</model>", "");
  return out.join("\n");
}

/** Colored 3MF: one object per group (per-object `displaycolor`). */
export function build3MFColored(
  previewPositions: Float32Array,
  groups: ColorGroup[],
): Uint8Array {
  const enc = new TextEncoder();
  return storeZip([
    { name: "[Content_Types].xml", data: enc.encode(CT_XML) },
    { name: "_rels/.rels", data: enc.encode(RELS_XML) },
    {
      name: "3D/3dmodel.model",
      data: enc.encode(threeMFColoredModel(previewPositions, groups)),
    },
  ]);
}

// CRC-32 (IEEE), table built once.
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(data: Uint8Array): number {
  let c = 0xffffffff;
  for (let i = 0; i < data.length; i++)
    c = CRC_TABLE[(c ^ data[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** Bundle named files into a store-only ZIP (e.g. animation frames). */
export function zipFiles(
  files: { name: string; data: Uint8Array }[],
): Uint8Array {
  return storeZip(files);
}

/** Minimal store-only (uncompressed) ZIP writer — matches the Rust one. */
function storeZip(files: { name: string; data: Uint8Array }[]): Uint8Array {
  const enc = new TextEncoder();
  const locals: Uint8Array[] = [];
  const central: Uint8Array[] = [];
  let offset = 0;
  for (const f of files) {
    const name = enc.encode(f.name);
    const crc = crc32(f.data);
    const lh = new DataView(new ArrayBuffer(30));
    lh.setUint32(0, 0x04034b50, true);
    lh.setUint16(4, 20, true);
    lh.setUint32(14, crc, true);
    lh.setUint32(18, f.data.length, true);
    lh.setUint32(22, f.data.length, true);
    lh.setUint16(26, name.length, true);
    const local = concat([new Uint8Array(lh.buffer), name, f.data]);
    locals.push(local);
    const ch = new DataView(new ArrayBuffer(46));
    ch.setUint32(0, 0x02014b50, true);
    ch.setUint16(4, 20, true);
    ch.setUint16(6, 20, true);
    ch.setUint32(16, crc, true);
    ch.setUint32(20, f.data.length, true);
    ch.setUint32(24, f.data.length, true);
    ch.setUint16(28, name.length, true);
    ch.setUint32(42, offset, true);
    central.push(concat([new Uint8Array(ch.buffer), name]));
    offset += local.length;
  }
  const cdStart = offset;
  const cd = concat(central);
  const eocd = new DataView(new ArrayBuffer(22));
  eocd.setUint32(0, 0x06054b50, true);
  eocd.setUint16(8, files.length, true);
  eocd.setUint16(10, files.length, true);
  eocd.setUint32(12, cd.length, true);
  eocd.setUint32(16, cdStart, true);
  return concat([...locals, cd, new Uint8Array(eocd.buffer)]);
}

function concat(parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

export function downloadBlob(data: Uint8Array, filename: string) {
  const blob = new Blob([data as BlobPart], {
    type: "application/octet-stream",
  });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
