import { describe, it, expect } from "vitest";
import {
  COMMANDS,
  paletteIds,
  shortcutRows,
  resolveCommands,
  displayKey,
  parseCombo,
} from "./index";
import type { Ctx } from "./types";

const CTX: Ctx = {
  rendering: false,
  engineKind: "openrscad",
  exportFmt: "stl",
};

describe("command registry invariants", () => {
  it("has unique ids", () => {
    const ids = COMMANDS.map((c) => c.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("has no duplicate (combo, scope) key binding", () => {
    const keys = COMMANDS.filter((c) => c.key).map(
      (c) => `${c.key!.combo}::${c.key!.scope}`,
    );
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("palette excludes palette-open and transport entries", () => {
    expect(paletteIds()).not.toContain("palette");
    expect(paletteIds().length).toBeGreaterThan(0);
  });

  it("shortcut rows exclude native-only accelerators", () => {
    const rows = shortcutRows(CTX);
    // download-scad (owner:web) is present; nothing native-only leaks in.
    expect(rows.some(([, title]) => title === "Download .scad")).toBe(true);
    expect(rows.length).toBe(
      COMMANDS.filter((c) => c.key && (c.key.owner ?? "both") !== "native")
        .length,
    );
  });

  it("resolveCommands drops ids with no handler and keeps ones with", () => {
    const cmds = resolveCommands({ render: () => {}, fit: () => {} });
    expect(cmds.map((c) => c.id).sort()).toEqual(["fit", "render"]);
    expect(typeof cmds[0].run).toBe("function");
  });
});

describe("keys formatting", () => {
  it("parses a combo into mods + key", () => {
    expect(parseCombo("Mod-Shift-f")).toMatchObject({
      mod: true,
      shift: true,
      key: "f",
    });
  });

  it("renders a display string (letters upper-cased)", () => {
    // Mac vs non-mac differ in separators/glyphs; assert the key letter shows.
    expect(displayKey({ combo: "Mod-Shift-f", scope: "global" })).toMatch(/F$/);
  });
});
