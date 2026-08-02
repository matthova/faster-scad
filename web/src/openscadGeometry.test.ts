import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { buildColoredFromOff, DEFAULT_COLOR, parseBinaryStl, measure } from "./openscadGeometry";

// A real OpenSCAD (Manifold backend) OFF export of three cubes — red, a
// semi-transparent blue, and one uncolored — captured verbatim from the vendored
// wasm. Guards that the F5-style colored-preview parsing keeps working.
const OFF = readFileSync(
  fileURLToPath(new URL("./__fixtures__/openscad-colored.off", import.meta.url)),
  "utf8",
);

describe("buildColoredFromOff", () => {
  const built = buildColoredFromOff(OFF);

  it("splits faces into one group per color", () => {
    expect(built.groups.length).toBe(3);
  });

  it("recovers the color() colors (0–1) including alpha", () => {
    const colors = built.groups.map((g) => g.color.map((c) => Math.round(c * 100) / 100));
    // red opaque, blue at ~0.5 alpha, and the default gold for the uncolored cube
    expect(colors).toContainEqual([1, 0, 0, 1]);
    expect(colors).toContainEqual([0, 0, 1, 0.5]);
    const gold = DEFAULT_COLOR.map((c) => Math.round(c * 100) / 100);
    expect(colors).toContainEqual(gold);
  });

  it("produces a contiguous, gap-free vertex layout matching the soup", () => {
    let cursor = 0;
    for (const g of built.groups) {
      expect(g.start).toBe(cursor); // groups tile the buffer with no gaps/overlap
      expect(g.count % 3).toBe(0); // whole triangles
      cursor += g.count;
    }
    expect(cursor).toBe(built.positions.length / 3); // covers every vertex
    expect(built.normals.length).toBe(built.positions.length);
  });

  it("gives each cube a positive volume (~1000 each, 3000 total)", () => {
    const { volume } = measure(built.positions);
    expect(volume).toBeGreaterThan(2900);
    expect(volume).toBeLessThan(3100);
  });

  it("emits unit-length normals", () => {
    const n = built.normals;
    for (let i = 0; i < n.length; i += 3) {
      const len = Math.hypot(n[i], n[i + 1], n[i + 2]);
      expect(len).toBeCloseTo(1, 4);
    }
  });
});

describe("parseBinaryStl", () => {
  it("round-trips a one-triangle binary STL", () => {
    const buf = new ArrayBuffer(84 + 50);
    const dv = new DataView(buf);
    dv.setUint32(80, 1, true);
    // normal (0,0,1) then verts (0,0,0)(1,0,0)(0,1,0)
    const floats = [0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0];
    floats.forEach((f, i) => dv.setFloat32(84 + i * 4, f, true));
    const { positions, normals } = parseBinaryStl(buf);
    expect(Array.from(positions)).toEqual([0, 0, 0, 1, 0, 0, 0, 1, 0]);
    expect(Array.from(normals.slice(0, 3))).toEqual([0, 0, 1]);
  });

  it("rejects a truncated buffer", () => {
    const buf = new ArrayBuffer(84 + 50);
    new DataView(buf).setUint32(80, 5, true); // claims 5 triangles, only 1 present
    expect(() => parseBinaryStl(buf)).toThrow();
  });
});
