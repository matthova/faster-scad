import { describe, it, expect } from "vitest";
import { parseSchema, toLiteral, sameShape, type Param } from "./customizer";

describe("parseSchema", () => {
  it("returns the params array from a well-formed schema", () => {
    const json = JSON.stringify({
      params: [
        { name: "w", group: "", description: null, type: "number", value: 10, control: { kind: "number" } },
      ],
    });
    const ps = parseSchema(json);
    expect(ps).toHaveLength(1);
    expect(ps[0].name).toBe("w");
  });

  it("returns [] for invalid JSON", () => {
    expect(parseSchema("{not json")).toEqual([]);
  });

  it("returns [] when `params` is missing or not an array", () => {
    expect(parseSchema("{}")).toEqual([]);
    expect(parseSchema(JSON.stringify({ params: 5 }))).toEqual([]);
  });
});

describe("toLiteral", () => {
  it("renders each value type as the literal the engine parses", () => {
    expect(toLiteral(30)).toBe("30");
    expect(toLiteral(-2.5)).toBe("-2.5");
    expect(toLiteral(true)).toBe("true");
    expect(toLiteral(false)).toBe("false");
    // strings are quoted + escaped
    expect(toLiteral('hi "there"')).toBe('"hi \\"there\\""');
    // vectors are bracketed, comma-joined
    expect(toLiteral([1, 2, 3])).toBe("[1,2,3]");
    expect(toLiteral([])).toBe("[]");
  });
});

describe("sameShape", () => {
  const p = (over: Partial<Param>): Param => ({
    name: "x",
    group: "",
    description: null,
    type: "number",
    value: 0,
    control: { kind: "number" },
    ...over,
  });

  it("is true when name, type, and control kind all match (values may differ)", () => {
    expect(sameShape([p({ value: 1 })], [p({ value: 999 })])).toBe(true);
  });

  it("is false on length, name, type, or control-kind differences", () => {
    expect(sameShape([p({})], [])).toBe(false);
    expect(sameShape([p({ name: "a" })], [p({ name: "b" })])).toBe(false);
    expect(sameShape([p({ type: "number" })], [p({ type: "string" })])).toBe(false);
    expect(
      sameShape(
        [p({ control: { kind: "number" } })],
        [p({ control: { kind: "slider", min: 0, max: 1, step: null } })],
      ),
    ).toBe(false);
  });
});
