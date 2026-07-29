import { describe, it, expect } from "vitest";
import { EditorState } from "@codemirror/state";
import { CompletionContext } from "@codemirror/autocomplete";
import { ensureSyntaxTree } from "@codemirror/language";
import { openscad } from "./openscad";
import { openscadCompletion } from "./complete";

/** Build an OpenSCAD editor state, force a full parse, and run the completion
 *  source at `pos` (end of doc by default). `explicit` mimics Ctrl-Space. */
function complete(doc: string, pos = doc.length, explicit = true) {
  const state = EditorState.create({ doc, extensions: [openscad()] });
  ensureSyntaxTree(state, doc.length, 5000);
  return openscadCompletion(new CompletionContext(state, pos, explicit));
}

function labels(doc: string, pos?: number, explicit?: boolean): string[] {
  return complete(doc, pos, explicit)?.options.map((o) => o.label) ?? [];
}

describe("openscadCompletion", () => {
  it("offers builtin modules and functions", () => {
    const l = labels("");
    expect(l).toContain("cube");
    expect(l).toContain("cylinder");
    expect(l).toContain("sphere");
    expect(l).toContain("linear_extrude");
  });

  it("offers keywords and special variables", () => {
    const l = labels("");
    expect(l).toContain("module");
    expect(l).toContain("function");
    expect(l).toContain("$fn");
    expect(l).toContain("$vpr");
  });

  it("carries a builtin's signature as detail and doc as info", () => {
    const cube = complete("")?.options.find((o) => o.label === "cube");
    expect(cube?.type).toBe("function");
    expect(cube?.detail).toBe("cube(size, center=false)");
    expect(typeof cube?.info).toBe("string");
  });

  it("types special variables as variables", () => {
    const fn = complete("")?.options.find((o) => o.label === "$fn");
    expect(fn?.type).toBe("variable");
  });

  it("surfaces a user module with its parameter list as detail", () => {
    const widget = complete("module widget(a, b) {}\n")?.options.find(
      (o) => o.label === "widget",
    );
    expect(widget?.type).toBe("function");
    expect(widget?.detail).toBe("(a, b)");
  });

  it("surfaces user assignments and parameters as variables", () => {
    const opts = complete("size = 5;\nmodule m(depth) {}\n")?.options ?? [];
    expect(opts.find((o) => o.label === "size")?.type).toBe("variable");
    expect(opts.find((o) => o.label === "depth")?.type).toBe("variable");
  });

  it("dedups a user symbol over a same-named builtin (one entry, user wins)", () => {
    const opts = complete("module render() {}\n")?.options ?? [];
    const renders = opts.filter((o) => o.label === "render");
    expect(renders).toHaveLength(1);
    expect(renders[0].detail).toBe("()"); // the user's empty param list, not the builtin sig
  });

  it("filters by a `$` prefix so special vars trigger", () => {
    const r = complete("$f");
    expect(r).not.toBeNull();
    expect(r!.from).toBe(0); // completes from the start of `$f`
    expect(r!.options.some((o) => o.label === "$fn")).toBe(true);
  });

  it("returns null at an empty, non-explicit boundary", () => {
    expect(complete("", 0, false)).toBeNull();
  });
});
