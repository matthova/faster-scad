// Customizer schema types (mirroring quito-wasm's JSON) plus the conversion
// from a UI value to the literal string the engine expects as an override.

export type ParamType = "number" | "bool" | "string" | "vector";

export interface Choice {
  value: number | string;
  label: string;
}

export type Control =
  | { kind: "number" }
  | { kind: "checkbox" }
  | { kind: "slider"; min: number; max: number; step: number | null }
  | { kind: "text"; maxLength: number | null }
  | { kind: "vector"; length: number }
  | { kind: "dropdown"; options: Choice[] };

export type ParamValue = number | boolean | string | number[];

export interface Param {
  name: string;
  group: string;
  description: string | null;
  type: ParamType;
  value: ParamValue;
  control: Control;
}

export function parseSchema(json: string): Param[] {
  try {
    const obj = JSON.parse(json);
    return Array.isArray(obj.params) ? (obj.params as Param[]) : [];
  } catch {
    return [];
  }
}

/** Render a UI value as the literal string the engine parses as an override. */
export function toLiteral(v: ParamValue): string {
  if (typeof v === "number") return String(v);
  if (typeof v === "boolean") return v ? "true" : "false";
  if (typeof v === "string") return JSON.stringify(v); // quoted + escaped
  return "[" + v.map((n) => String(n)).join(",") + "]"; // vector of numbers
}

/** True when two schemas describe the same controls (name+type+control shape),
 *  so the current override values can be carried across a re-parse. */
export function sameShape(a: Param[], b: Param[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((p, i) => {
    const q = b[i];
    return p.name === q.name && p.type === q.type && p.control.kind === q.control.kind;
  });
}
