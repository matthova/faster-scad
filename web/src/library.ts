// Lazy library resolution. Given the main source plus the playground's local
// files, compute the transitive closure of `include`/`use` targets, fetching
// any that aren't local — from a registered CDN (e.g. BOSL2) by path prefix, or
// from the app's bundled `/lib/` folder otherwise. Fetched files are cached, so
// only the first render that needs a library pays the network cost.

export interface NamedFile {
  name: string;
  content: string;
}

/** Path-prefix → CDN base for known libraries (BSD/permissive, CORS-enabled). */
const LIBRARIES: Record<string, string> = {
  // BOSL2 (BSD-2): `include <BOSL2/std.scad>` → jsDelivr.
  BOSL2: "https://cdn.jsdelivr.net/gh/BelfrySCAD/BOSL2@3.0.0/",
};

// resolved path → content, or null when known-missing (don't refetch).
const cache = new Map<string, string | null>();

/** Extract the `<...>` targets of `include`/`use` statements. */
export function extractPaths(src: string): string[] {
  const re = /\b(?:include|use)\s*<([^>\n]+)>/g;
  const out: string[] = [];
  let m: RegExpExecArray | null;
  while ((m = re.exec(src)) !== null) out.push(m[1].trim());
  return out;
}

function dirname(p: string): string {
  const i = p.lastIndexOf("/");
  return i < 0 ? "" : p.slice(0, i);
}

/** Resolve `path` (as written in a file whose directory is `dir`) to a
 *  normalized project path, collapsing `.`/`..` segments. */
export function normJoin(dir: string, path: string): string {
  const combined = dir && !path.startsWith("/") ? `${dir}/${path}` : path;
  const parts: string[] = [];
  for (const seg of combined.split("/")) {
    if (seg === "" || seg === ".") continue;
    if (seg === "..") parts.pop();
    else parts.push(seg);
  }
  return parts.join("/");
}

async function fetchLib(resolved: string, libBase: string): Promise<string | null> {
  if (cache.has(resolved)) return cache.get(resolved)!;
  const top = resolved.split("/")[0];
  const url = LIBRARIES[top]
    ? LIBRARIES[top] + resolved.slice(top.length + 1)
    : libBase + resolved;
  let text: string | null = null;
  try {
    const r = await fetch(url);
    text = r.ok ? await r.text() : null;
  } catch {
    text = null;
  }
  cache.set(resolved, text);
  return text;
}

/** The full file set (local + fetched) needed to render `mainSource`. */
export async function resolveClosure(
  mainSource: string,
  localFiles: NamedFile[],
  libBase: string,
): Promise<{ names: string[]; contents: string[] }> {
  const map = new Map<string, string>();
  for (const f of localFiles) map.set(f.name, f.content);

  const seen = new Set<string>();
  const queue: Array<[string, string]> = []; // [path, includingDir]
  const enqueue = (content: string, dir: string) => {
    for (const p of extractPaths(content)) queue.push([p, dir]);
  };
  enqueue(mainSource, "");
  for (const f of localFiles) enqueue(f.content, dirname(f.name));

  while (queue.length) {
    const [path, dir] = queue.shift()!;
    const resolved = normJoin(dir, path);
    if (map.has(resolved) || seen.has(resolved)) continue;
    seen.add(resolved);
    const content = await fetchLib(resolved, libBase);
    if (content == null) continue; // missing → leave out; the engine warns
    map.set(resolved, content);
    enqueue(content, dirname(resolved));
  }

  const names: string[] = [];
  const contents: string[] = [];
  for (const [k, v] of map) {
    names.push(k);
    contents.push(v);
  }
  return { names, contents };
}
