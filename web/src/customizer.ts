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

/** Coerce a raw OpenSCAD parameter-set string to a `ParamValue`, using the
 *  schema param's type (text keeps its raw string; else number/bool/vector). */
export function coerceSetValue(raw: string, type: ParamType | undefined): ParamValue {
  if (type === "string") return raw;
  if (type === "bool") return raw === "true";
  if (type === "vector") {
    const inner = raw.trim().replace(/^\[|\]$/g, "");
    return inner
      .split(",")
      .map((s) => Number(s.trim()))
      .filter((n) => !Number.isNaN(n));
  }
  if (type === "number") {
    const n = Number(raw);
    return Number.isNaN(n) ? raw : n;
  }
  // Unknown param: best-effort (number → bool → keep string).
  if (raw === "true" || raw === "false") return raw === "true";
  const n = Number(raw);
  return raw.trim() !== "" && !Number.isNaN(n) ? n : raw;
}

/** Serialize named parameter sets to OpenSCAD's `.json` format (every value is a
 *  string; text is stored raw). */
export function toParamSetsJson(sets: Record<string, Record<string, ParamValue>>): string {
  const raw = (v: ParamValue): string =>
    typeof v === "string" ? v : typeof v === "boolean" ? String(v) : toLiteral(v);
  const parameterSets: Record<string, Record<string, string>> = {};
  for (const [name, vals] of Object.entries(sets)) {
    parameterSets[name] = Object.fromEntries(
      Object.entries(vals).map(([k, v]) => [k, raw(v)]),
    );
  }
  return JSON.stringify({ fileFormatVersion: "1", parameterSets }, null, 2);
}

/** Parse an OpenSCAD `.json` parameter-set file into named override maps,
 *  coercing each value by the current schema's types. */
export function fromParamSetsJson(
  json: string,
  schema: Param[],
): Record<string, Record<string, ParamValue>> {
  const typeOf = new Map(schema.map((p) => [p.name, p.type]));
  const obj = JSON.parse(json);
  const psets = (obj?.parameterSets ?? {}) as Record<string, Record<string, string>>;
  const out: Record<string, Record<string, ParamValue>> = {};
  for (const [setName, vals] of Object.entries(psets)) {
    const m: Record<string, ParamValue> = {};
    for (const [k, v] of Object.entries(vals)) {
      m[k] = coerceSetValue(String(v), typeOf.get(k));
    }
    out[setName] = m;
  }
  return out;
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
