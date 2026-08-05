// Turns a render's provenance channel into the Objects list: one row per leaf
// statement that produced geometry, labelled with its source and carrying its
// triangle count. Rows drive the isolate feature (§6) — clicking one shows only
// that node's geometry.
//
// SPIKE NOTE: this is provenance-derived (geometry-first). The final version
// (Track E §7) parses files[0] with the Lezer grammar for the *structural* tree
// and joins triangle counts on by span, so operations that produce no leaf
// (a difference()'s tool operands, a collapsed hull/minkowski subtree) still
// show as rows reading "no geometry". Provenance can only surface surviving
// leaves — which is exactly what's isolatable — so it's the right basis for the
// spike; the Lezer outline is the iterate step.
import type { ProvenanceGroup, Span } from "./viewer";

export interface ObjectRow {
  /** Innermost (leaf) span — what isolate targets (UTF-8 byte offsets). */
  span: Span;
  /** First non-empty line of the span's source, trimmed for display. */
  label: string;
  triangles: number;
  /** Byte offset, for stable ordering. */
  start: number;
}

/** Maps a UTF-8 byte offset (engine spans) to a UTF-16 index (JS strings). */
export type ByteToChar = (byte: number) => number;

/** A short label: the first non-blank line of the span's source, trimmed and
 *  capped so a multi-line call reads as one row. Spans are UTF-8 byte offsets,
 *  so convert to char indices first — the default file's em-dash alone shifts
 *  every later offset by 2. */
function labelFor(source: string, span: Span, toChar: ByteToChar): string {
  const from = toChar(span[0]);
  const to = toChar(span[1]);
  if (from < 0 || to <= from || to > source.length) return "(geometry)";
  const text = source.slice(from, to).trim();
  if (!text) return "(geometry)";
  const firstLine = text.split("\n")[0].trim();
  return firstLine.length > 48 ? firstLine.slice(0, 47) + "…" : firstLine;
}

/** Build the deduplicated Objects rows for a render. Groups sharing an innermost
 *  span (a reused module, a `for` loop's instances) collapse into one row whose
 *  triangle count sums the instances — matching isolate, which shows every group
 *  under the span. Unattributable groups (empty stack) collapse into one
 *  "(library geometry)" row. */
export function buildObjectRows(
  groups: ProvenanceGroup[],
  source: string,
  toChar: ByteToChar = (b) => b,
): ObjectRow[] {
  const byKey = new Map<string, ObjectRow>();
  const LIB_KEY = "lib";
  for (const g of groups) {
    const span = g.spans[g.spans.length - 1];
    const tris = g.count / 3;
    if (!span) {
      const row = byKey.get(LIB_KEY);
      if (row) row.triangles += tris;
      else
        byKey.set(LIB_KEY, {
          span: [-1, -1],
          label: "(library geometry)",
          triangles: tris,
          start: Number.MAX_SAFE_INTEGER, // sort last
        });
      continue;
    }
    const key = `${span[0]}:${span[1]}`;
    const row = byKey.get(key);
    if (row) row.triangles += tris;
    else
      byKey.set(key, {
        span,
        label: labelFor(source, span, toChar),
        triangles: tris,
        start: span[0],
      });
  }
  return [...byKey.values()].sort((a, b) => a.start - b.start);
}
