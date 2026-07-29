import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { BUILTINS } from "./builtins";

// Police TS/Rust parity: builtins.ts is a hand-port of the Rust LSP table, so a
// name/kind that drifts in one and not the other should fail here. Parses the
// `b!("name", module|function, ...)` macro invocations out of the Rust source.
const RUST = fileURLToPath(
  new URL("../../../crates/quito-lsp/src/builtins.rs", import.meta.url),
);

function rustBuiltins(): { name: string; isModule: boolean }[] {
  const src = readFileSync(RUST, "utf8");
  const re = /b!\(\s*"([^"]+)"\s*,\s*(module|function)\b/g;
  const out: { name: string; isModule: boolean }[] = [];
  for (let m = re.exec(src); m; m = re.exec(src)) {
    out.push({ name: m[1], isModule: m[2] === "module" });
  }
  return out;
}

describe("builtins TS/Rust parity", () => {
  const rust = rustBuiltins();

  it("parses a plausible number of Rust entries", () => {
    expect(rust.length).toBeGreaterThan(50);
  });

  it("has the same set of names in both", () => {
    const ts = BUILTINS.map((b) => b.name).sort();
    const rs = rust.map((b) => b.name).sort();
    expect(ts).toEqual(rs);
  });

  it("agrees on module vs function for every name", () => {
    const rsKind = new Map(rust.map((b) => [b.name, b.isModule]));
    for (const b of BUILTINS) {
      expect(b.isModule, `${b.name} module/function kind`).toBe(rsKind.get(b.name));
    }
  });

  it("has no duplicate names in the TS table", () => {
    const names = BUILTINS.map((b) => b.name);
    expect(new Set(names).size).toBe(names.length);
  });
});
