import { describe, it, expect } from "vitest";
import {
  buildBinarySTL,
  buildOFF,
  buildOBJ,
  buildAMF,
  build3MF,
  build3MFColored,
} from "./stl";

// A single triangle in the XY plane (CCW → +Z normal), integer coords so f32
// round-trips exactly and the text output is stable for golden comparison.
const TRI = new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]);
const text = (u: Uint8Array) => new TextDecoder().decode(u);

describe("buildBinarySTL", () => {
  it("writes the STL binary layout for one triangle", () => {
    const out = buildBinarySTL(TRI);
    expect(out.length).toBe(84 + 50); // 80B header + u32 count + one 50B tri
    const dv = new DataView(out.buffer);
    expect(dv.getUint32(80, true)).toBe(1); // triangle count
    // face normal (offset 84) is the CCW +Z unit normal
    expect(dv.getFloat32(84, true)).toBeCloseTo(0);
    expect(dv.getFloat32(88, true)).toBeCloseTo(0);
    expect(dv.getFloat32(92, true)).toBeCloseTo(1);
    // the 9 vertex floats (offset 96) echo the input positions
    for (let k = 0; k < 9; k++) {
      expect(dv.getFloat32(96 + k * 4, true)).toBe(TRI[k]);
    }
  });
});

describe("buildOFF / buildOBJ (welded, exact golden)", () => {
  it("emits exact OFF for one triangle", () => {
    expect(text(buildOFF(TRI))).toBe("OFF\n3 1 0\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n");
  });

  it("emits exact OBJ (1-based faces) for one triangle", () => {
    expect(text(buildOBJ(TRI))).toBe("# exported by Quito\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n");
  });

  it("welds shared vertices across triangles", () => {
    // Two triangles forming a unit quad share two vertices → 4 unique verts.
    const quad = new Float32Array([
      0, 0, 0, 1, 0, 0, 1, 1, 0, // t0
      0, 0, 0, 1, 1, 0, 0, 1, 0, // t1 (shares (0,0,0) and (1,1,0))
    ]);
    expect(text(buildOFF(quad)).split("\n")[1]).toBe("4 2 0"); // 4 verts, 2 faces
  });
});

describe("buildAMF", () => {
  it("emits welded indexed XML", () => {
    const s = text(buildAMF(TRI));
    expect(s.startsWith('<?xml version="1.0" encoding="UTF-8"?>\n<amf unit="millimeter">')).toBe(true);
    expect(s).toContain("<vertex><coordinates><x>0</x><y>0</y><z>0</z></coordinates></vertex>");
    expect(s).toContain("<triangle><v1>0</v1><v2>1</v2><v3>2</v3></triangle>");
    expect(s.trimEnd().endsWith("</amf>")).toBe(true);
  });
});

describe("build3MF", () => {
  it("produces a store-only ZIP whose parts are readable inline", () => {
    const zip = build3MF(TRI);
    // ZIP local file header magic.
    expect(Array.from(zip.slice(0, 4))).toEqual([0x50, 0x4b, 0x03, 0x04]);
    // EOCD magic near the end.
    const s = text(zip);
    expect(s).toContain("[Content_Types].xml");
    expect(s).toContain("3D/3dmodel.model");
    // Store-only → the model XML is uncompressed and directly present.
    expect(s).toContain('<vertex x="0" y="0" z="0"/>');
    expect(s).toContain('<triangle v1="0" v2="1" v3="2"/>');
  });
});

describe("build3MFColored", () => {
  it("emits one displaycolor base + object per group", () => {
    const zip = build3MFColored(TRI, [
      { start: 0, count: 3, color: [1, 0, 0, 1], mode: "solid" },
    ]);
    const s = text(zip);
    expect(Array.from(zip.slice(0, 4))).toEqual([0x50, 0x4b, 0x03, 0x04]);
    expect(s).toContain('<base name="c0" displaycolor="#FF0000FF"/>');
    expect(s).toContain('pid="1" pindex="0"');
  });
});
