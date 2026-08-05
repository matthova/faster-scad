import { describe, it, expect } from "vitest";
import { buildObjectRows } from "./objectTree";
import type { ProvenanceGroup } from "./viewer";

const src = "cube(10);\nsphere(5);\ntranslate([1,0,0]) cube(2);\n";
// byte spans into src:
const CUBE: [number, number] = [0, 9]; // "cube(10);"
const SPHERE: [number, number] = [10, 20]; // "sphere(5);"

describe("buildObjectRows", () => {
  it("one row per leaf, labelled from source, triangle counts from count/3", () => {
    const groups: ProvenanceGroup[] = [
      { start: 0, count: 36, spans: [CUBE] }, // 12 tris
      { start: 36, count: 60, spans: [SPHERE] }, // 20 tris
    ];
    const rows = buildObjectRows(groups, src);
    expect(rows.map((r) => r.label)).toEqual(["cube(10);", "sphere(5);"]);
    expect(rows.map((r) => r.triangles)).toEqual([12, 20]);
  });

  it("groups sharing an innermost span collapse into one row, summing tris", () => {
    const groups: ProvenanceGroup[] = [
      { start: 0, count: 30, spans: [CUBE] }, // a for-loop instance
      { start: 30, count: 30, spans: [CUBE] }, // another instance, same span
    ];
    const rows = buildObjectRows(groups, src);
    expect(rows).toHaveLength(1);
    expect(rows[0].triangles).toBe(20); // (30+30)/3
  });

  it("unattributable groups collapse into one library row, sorted last", () => {
    const groups: ProvenanceGroup[] = [
      { start: 0, count: 30, spans: [] },
      { start: 30, count: 36, spans: [CUBE] },
    ];
    const rows = buildObjectRows(groups, src);
    expect(rows.map((r) => r.label)).toEqual([
      "cube(10);",
      "(library geometry)",
    ]);
  });

  it("applies byte→char conversion so multi-byte chars don't shift labels", () => {
    // A leading em-dash (— = 3 UTF-8 bytes, 1 JS char) pushes byte offsets 2
    // ahead of char offsets for everything after it.
    const s = "// —\ncube(9);\n"; // "cube(9);" is chars 5..13, bytes 7..15
    const toChar = (byte: number) => (byte >= 7 ? byte - 2 : byte);
    const groups: ProvenanceGroup[] = [
      { start: 0, count: 36, spans: [[7, 15]] },
    ];
    const rows = buildObjectRows(groups, s, toChar);
    expect(rows[0].label).toBe("cube(9);"); // not "be(9);"
  });

  it("orders rows by source position", () => {
    const groups: ProvenanceGroup[] = [
      { start: 0, count: 3, spans: [SPHERE] },
      { start: 3, count: 3, spans: [CUBE] },
    ];
    const rows = buildObjectRows(groups, src);
    expect(rows.map((r) => r.label)).toEqual(["cube(10);", "sphere(5);"]);
  });
});
