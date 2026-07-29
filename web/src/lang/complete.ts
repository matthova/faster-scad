// Autocompletion source for OpenSCAD: built-in modules/functions, language
// keywords, and user-defined symbols (modules, functions, assignments, and
// parameters) scraped from the Lezer syntax tree.
import type {
  Completion,
  CompletionContext,
  CompletionResult,
} from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import { BUILTINS } from "./builtins";

// Language keywords not otherwise surfaced as builtins. `if`/`for`/`let` also
// appear in BUILTINS (with richer signatures), so they're intentionally omitted
// here to avoid duplicate entries.
const KEYWORDS = [
  "module",
  "function",
  "else",
  "each",
  "include",
  "use",
  "true",
  "false",
  "undef",
];

/** Walk the syntax tree and collect user-defined names as completions:
 *  module/function definitions (with their parameter list as detail) plus
 *  assignment targets and module/function parameters. */
function collectUserSymbols(ctx: CompletionContext): Completion[] {
  const out: Completion[] = [];
  const doc = ctx.state.doc;
  const text = (from: number, to: number) => doc.sliceString(from, to);
  const cursor = syntaxTree(ctx.state).cursor();
  do {
    const type = cursor.name;
    if (type === "ModuleDefinition" || type === "FunctionDefinition") {
      const node = cursor.node;
      const nameNode = node.getChild("VariableName");
      if (!nameNode) continue;
      const params = node.getChild("ParamList");
      out.push({
        label: text(nameNode.from, nameNode.to),
        type: "function",
        detail: params ? text(params.from, params.to) : undefined,
      });
    } else if (type === "Assignment" || type === "Parameter") {
      const nameNode = cursor.node.getChild("VariableName");
      if (nameNode) {
        out.push({ label: text(nameNode.from, nameNode.to), type: "variable" });
      }
    }
  } while (cursor.next());
  return out;
}

/** CodeMirror completion source for OpenSCAD. Triggers on identifier/`$var`
 *  prefixes (and explicitly via Ctrl-Space). */
export function openscadCompletion(ctx: CompletionContext): CompletionResult | null {
  const word = ctx.matchBefore(/\$?[\w]*/);
  if (!word || (word.from === word.to && !ctx.explicit)) return null;

  // Dedup by label across sources; later insertions win. Order: keywords, then
  // builtins (override the `if`/`for`/`let` keyword stubs with richer info),
  // then user symbols (override builtins on name collision).
  const byName = new Map<string, Completion>();
  for (const kw of KEYWORDS) byName.set(kw, { label: kw, type: "keyword" });
  for (const b of BUILTINS) {
    byName.set(b.name, {
      label: b.name,
      type: b.name.startsWith("$") ? "variable" : "function",
      detail: b.signature,
      info: b.doc,
    });
  }
  for (const u of collectUserSymbols(ctx)) byName.set(u.label, u);

  return {
    from: word.from,
    options: [...byName.values()],
    validFor: /^\$?[\w]*$/,
  };
}
