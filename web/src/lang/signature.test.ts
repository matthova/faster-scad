import { describe, it, expect } from "vitest";
import { EditorState } from "@codemirror/state";
import { ensureSyntaxTree } from "@codemirror/language";
import { openscad } from "./openscad";
import { signatureAt } from "./signature";

/** Resolve the signature at cursor position `pos` in `doc`. `pos` defaults to the
 *  first `|` marker in `doc` (which is stripped before parsing). */
function sigAt(doc: string, pos?: number) {
  if (pos === undefined) {
    pos = doc.indexOf("|");
    doc = doc.replace("|", "");
  }
  const state = EditorState.create({
    doc,
    selection: { anchor: pos },
    extensions: [openscad()],
  });
  ensureSyntaxTree(state, doc.length, 5000);
  return signatureAt(state);
}

describe("signatureAt", () => {
  it("shows a builtin module's signature inside its parens", () => {
    const s = sigAt("cube(|);");
    expect(s?.name).toBe("cube");
    expect(s?.signature).toBe("cube(size, center=false)");
  });

  it("shows a builtin function's signature inside a call expression", () => {
    const s = sigAt("x = sin(4|5);");
    expect(s?.name).toBe("sin");
    expect(s?.signature).toBe("sin(deg)");
  });

  it("keeps showing while typing arguments", () => {
    const s = sigAt("translate([1, 2, |3]);");
    expect(s?.name).toBe("translate");
  });

  it("resolves the innermost call when nested", () => {
    const s = sigAt("translate([0,0,0]) cube(1|);");
    expect(s?.name).toBe("cube");
  });

  it("hides once the cursor is past the closing paren", () => {
    expect(sigAt("cube()|;")).toBeNull();
  });

  it("hides before the opening paren / at column 0", () => {
    expect(sigAt("|cube();")).toBeNull();
    expect(sigAt("cube|();")).toBeNull();
  });

  it("shows a user-defined module's parameter list", () => {
    const s = sigAt("module widget(a, b) {}\nwidget(|);");
    expect(s?.signature).toBe("widget(a, b)");
    expect(s?.doc).toBe("user-defined module");
  });

  it("shows a user-defined function's parameter list", () => {
    const s = sigAt("function sq(x) = x*x;\ny = sq(|);");
    expect(s?.signature).toBe("sq(x)");
    expect(s?.doc).toBe("user-defined function");
  });

  it("returns null for an unknown call name", () => {
    expect(sigAt("nope(|);")).toBeNull();
  });
});
