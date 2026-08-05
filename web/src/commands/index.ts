// The command registry: one module-level list of pure metadata. App attaches a
// `run` per id (resolveCommands) to produce runnable Commands; the palette, help
// sheet, and keyboard-shortcut display are all projections over it. Keeping the
// metadata here (not inline JSX) is what makes controls countable.
import type { Command, CommandMeta, Ctx } from "./types";
import { titleOf } from "./types";
import { displayKey } from "./keys";

export * from "./types";
export { displayKey, matchesEvent, parseCombo } from "./keys";

// prettier-ignore
export const COMMANDS: CommandMeta[] = [
  { id: "render", title: "Render", key: { combo: "Mod-Enter", scope: "both", prec: "high" }, help: "Re-render the model" },
  { id: "stop", title: "Stop render", when: (c) => c.rendering, help: "Cancel the render in flight" },
  { id: "fit", title: "Zoom to fit", key: { combo: "Mod-Shift-f", scope: "global" }, help: "Frame the model without changing the angle" },
  { id: "reset-view", title: "Reset view", menu: "reset-view" },
  { id: "console", title: "Toggle console", key: { combo: "Mod-j", scope: "global" } },
  { id: "dock", title: "Toggle dock" },
  { id: "palette", title: "Command palette", key: { combo: "Mod-k", scope: "global" } },
  { id: "download-scad", title: "Download .scad", key: { combo: "Mod-s", scope: "both", owner: "web" } },
  { id: "grid", group: "display", title: "Toggle grid & axes" },
  { id: "edges", group: "display", title: "Toggle edge overlay" },
  { id: "dims", group: "display", title: "Toggle dimensions" },
  { id: "section", group: "display", title: "Toggle section plane" },
  { id: "fast", title: "Toggle fast preview" },
  { id: "engine", title: (c) => `Switch engine (${c.engineKind === "openscad" ? "→ Quito" : "→ OpenSCAD"})` },
  { id: "q-draft", group: "quality", title: "Quality: Draft" },
  { id: "q-normal", group: "quality", title: "Quality: Normal" },
  { id: "q-fine", group: "quality", title: "Quality: Fine" },
  { id: "png", group: "output", title: "Save PNG" },
  { id: "export", group: "output", title: (c) => `Export (${c.exportFmt.toUpperCase()})` },
  { id: "theme-auto", group: "app", title: "Theme: Auto (follow OS)" },
  { id: "theme-light", group: "app", title: "Theme: Light" },
  { id: "theme-dark", group: "app", title: "Theme: Dark" },
  { id: "help", group: "app", title: "Help & keyboard shortcuts" },
  { id: "new", group: "project", title: "New project", menu: "new" },
  { id: "share", group: "project", title: "Copy share link" },
];

const BY_ID = new Map(COMMANDS.map((m) => [m.id, m]));

/** Commands that appear in the ⌘K palette (everything except palette-open and
 *  transport-pinned entries). */
export function paletteIds(): string[] {
  return COMMANDS.filter(
    (m) => m.id !== "palette" && m.pin !== "transport",
  ).map((m) => m.id);
}

/** `[displayKey, title]` rows for the help sheet — every command with a key that
 *  the web build can actually invoke (native-only accelerators are excluded). */
export function shortcutRows(ctx: Ctx): [string, string][] {
  return COMMANDS.filter(
    (m) => m.key && (m.key.owner ?? "both") !== "native",
  ).map((m) => [displayKey(m.key!), titleOf(m, ctx)]);
}

/** Attach `run` handlers (keyed by id) to the metadata, dropping any id App
 *  didn't provide a handler for (e.g. `share` on desktop). */
export function resolveCommands(runs: Record<string, () => void>): Command[] {
  return COMMANDS.filter((m) => runs[m.id]).map((m) => ({
    ...m,
    run: runs[m.id],
  }));
}

export function commandMeta(id: string): CommandMeta | undefined {
  return BY_ID.get(id);
}
