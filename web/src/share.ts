// Shareable links: the whole project (files + parameter overrides) is
// serialized to JSON, lz-string-compressed, and stashed in the URL fragment —
// mirroring the TypeScript playground's `#code/<compressed>` scheme
// (https://www.typescriptlang.org/play/). Opening such a URL restores the
// project without any server round-trip; the data lives entirely in the link.
import {
  compressToEncodedURIComponent,
  decompressFromEncodedURIComponent,
} from "lz-string";
import type { ParamValue } from "./customizer";
import type { Project } from "./project";

const PREFIX = "#code/";

// Compact on-wire shape — short keys keep the encoded link small.
interface Wire {
  f: { n: string; c: string }[];
  o?: Record<string, ParamValue>;
}

/** Encode a project into the `#code/…` fragment payload (no leading `#code/`). */
export function encodeProject(p: Project): string {
  const wire: Wire = {
    f: p.files.map((f) => ({ n: f.name, c: f.content })),
    o: Object.keys(p.overrides).length ? p.overrides : undefined,
  };
  return compressToEncodedURIComponent(JSON.stringify(wire));
}

/** Build a full shareable URL for the current project, at the current page. */
export function shareUrl(p: Project): string {
  const { origin, pathname, search } = window.location;
  return `${origin}${pathname}${search}${PREFIX}${encodeProject(p)}`;
}

/** Decode a project from a URL hash, or null if it isn't a valid share link. */
export function decodeSharedProject(hash: string = window.location.hash): Project | null {
  if (!hash.startsWith(PREFIX)) return null;
  const raw = decompressFromEncodedURIComponent(hash.slice(PREFIX.length));
  if (!raw) return null;
  try {
    const w = JSON.parse(raw) as Wire;
    if (!Array.isArray(w.f)) return null;
    const files = w.f
      .filter((f) => f && typeof f.n === "string" && typeof f.c === "string")
      .map((f) => ({ name: f.n, content: f.c }));
    if (files.length === 0) return null;
    return {
      files,
      overrides: w.o && typeof w.o === "object" ? w.o : {},
      active: 0,
    };
  } catch {
    return null;
  }
}
