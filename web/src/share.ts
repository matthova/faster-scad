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

/** Animation playback state carried in a share link, so the recipient opens on
 *  the same frame ($t), speed (fps), range (steps), and play/pause state. */
export interface Anim {
  t?: number; // $t, 0–1
  fps?: number;
  steps?: number;
  playing?: boolean;
}

// Compact on-wire shape — short keys keep the encoded link small.
interface Wire {
  f: { n: string; c: string }[];
  o?: Record<string, ParamValue>;
  a?: Anim;
}

/** Encode a project into the `#code/…` fragment payload (no leading `#code/`).
 *  `anim` is omitted from the link when undefined, so default (non-animated)
 *  projects encode exactly as before. */
export function encodeProject(p: Project, anim?: Anim): string {
  const wire: Wire = {
    f: p.files.map((f) => ({ n: f.name, c: f.content })),
    o: Object.keys(p.overrides).length ? p.overrides : undefined,
    a: anim,
  };
  return compressToEncodedURIComponent(JSON.stringify(wire));
}

/** Build a full shareable URL for the current project, at the current page. */
export function shareUrl(p: Project, anim?: Anim): string {
  const { origin, pathname, search } = window.location;
  return `${origin}${pathname}${search}${PREFIX}${encodeProject(p, anim)}`;
}

/** A decoded share link: the project plus any animation playback state. */
export type SharedProject = Project & { anim?: Anim };

/** Decode a project from a URL hash, or null if it isn't a valid share link. */
export function decodeSharedProject(hash: string = window.location.hash): SharedProject | null {
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
      anim: decodeAnim(w.a),
    };
  } catch {
    return null;
  }
}

/** Validate/normalize the animation payload; undefined if absent or malformed. */
function decodeAnim(a: unknown): Anim | undefined {
  if (!a || typeof a !== "object") return undefined;
  const src = a as Record<string, unknown>;
  const anim: Anim = {};
  if (typeof src.t === "number" && isFinite(src.t)) anim.t = src.t;
  if (typeof src.fps === "number" && isFinite(src.fps)) anim.fps = src.fps;
  if (typeof src.steps === "number" && isFinite(src.steps)) anim.steps = src.steps;
  if (typeof src.playing === "boolean") anim.playing = src.playing;
  return Object.keys(anim).length ? anim : undefined;
}
