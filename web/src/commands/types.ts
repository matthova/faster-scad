// A control is a CommandDef, not a piece of JSX. The topbar, command palette,
// help sheet, and keyboard shortcuts are all projections over one registry, so a
// control becomes *countable* (Track E §2). Metadata is module-level and pure
// (testable without React); the imperative `run` handlers are supplied by App,
// keyed by id, because they close over live refs.
import type { EngineKind } from "../prefs";
import type { ExportFmt } from "../renderState";

/** Popover / dock groups a command can live under. */
export type Group = "project" | "display" | "quality" | "output" | "app";

/** Where a keyboard binding is active. */
export type KeyScope = "editor" | "global" | "both";

/** A keyboard binding, canonical form. `combo` uses CodeMirror's spelling
 *  ("Mod-Enter", "Mod-Shift-f"); keys.ts formats it for display and matching.
 *  The three axes matter: `scope` (an editor binding must not also fire a window
 *  handler), `prec` (Mod-Enter must beat basicSetup's insertBlankLine), and
 *  `owner` (a native accelerator must not double-fire with a window handler on
 *  desktop). */
export interface KeySpec {
  combo: string;
  scope: KeyScope;
  prec?: "high" | "default";
  owner?: "web" | "native" | "both";
}

/** The (capped) state a command's title/when/disabledReason may read. Keep this
 *  small — a 30-field bag is the review-time failure (Track E §7). */
export interface Ctx {
  rendering: boolean;
  engineKind: EngineKind;
  exportFmt: ExportFmt;
}

/** Pure, module-level metadata for one command. `run` is attached by App. */
export interface CommandMeta {
  id: string;
  /** Group popover this belongs to (Phase 4 renders them). */
  group?: Group;
  /** Pinned directly to a surface rather than living in a group. */
  pin?: "topbar" | "transport";
  /** Static title, or one derived from context (engine/export are dynamic). */
  title: string | ((c: Ctx) => string);
  /** One-line help for the help sheet / tooltip. */
  help?: string;
  /** Keyboard binding, if any. */
  key?: KeySpec;
  /** Native-menu id (must match the Rust build_menu ids). */
  menu?: string;
  /** Shown only when this returns true (default: always). */
  when?: (c: Ctx) => boolean;
  /** When present and truthy, the command is shown disabled with this reason
   *  (instead of hidden — hiding loses discoverability, Track E §4). */
  disabledReason?: (c: Ctx) => string;
}

/** A command ready to render/run: metadata + its resolved handler. */
export interface Command extends CommandMeta {
  run: () => void;
}

export function titleOf(m: CommandMeta, c: Ctx): string {
  return typeof m.title === "function" ? m.title(c) : m.title;
}
