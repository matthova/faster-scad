// Signature help (parameter hints) for OpenSCAD: while the cursor sits inside a
// call's parentheses, a tooltip above the line shows that module/function's
// signature and doc. Distinct from autocomplete (which offers word completions
// and closes once you're no longer typing an identifier); the two coexist —
// completion below the cursor, this hint above.
import {
  StateField,
  type EditorState,
  type Extension,
} from "@codemirror/state";
import { EditorView, showTooltip, type Tooltip } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import type { SyntaxNode } from "@lezer/common";
import { BUILTINS } from "./builtins";

const BUILTIN_BY_NAME = new Map(BUILTINS.map((b) => [b.name, b]));

export interface SignatureInfo {
  name: string;
  signature: string;
  doc: string;
}

/** Slice a user-defined module/function's signature (`name(params)`) from the
 *  tree, or null if no matching definition exists. Mirrors the tree-cursor scan
 *  in complete.ts's collectUserSymbols. */
function findUserSignature(
  state: EditorState,
  name: string,
): SignatureInfo | null {
  const cursor = syntaxTree(state).cursor();
  do {
    if (
      cursor.name === "ModuleDefinition" ||
      cursor.name === "FunctionDefinition"
    ) {
      const node = cursor.node;
      const nameNode = node.getChild("VariableName");
      if (
        nameNode &&
        state.doc.sliceString(nameNode.from, nameNode.to) === name
      ) {
        const params = node.getChild("ParamList");
        const kind = cursor.name === "ModuleDefinition" ? "module" : "function";
        return {
          name,
          signature:
            name +
            (params ? state.doc.sliceString(params.from, params.to) : "()"),
          doc: `user-defined ${kind}`,
        };
      }
    }
  } while (cursor.next());
  return null;
}

/** The signature of the call whose argument list encloses the cursor, or null
 *  when the cursor isn't between a call's parens. Builtins first, then a
 *  user-defined module/function of the same name. Pure (no DOM) — testable. */
export function signatureAt(state: EditorState): SignatureInfo | null {
  const head = state.selection.main.head;
  const tree = syntaxTree(state);

  // Nearest enclosing ArgList (walk up from the node at the cursor).
  let argList: SyntaxNode | null = null;
  for (
    let n: SyntaxNode | null = tree.resolveInner(head, -1);
    n;
    n = n.parent
  ) {
    if (n.name === "ArgList") {
      argList = n;
      break;
    }
  }
  if (!argList) return null;

  // Only while strictly inside the parens. `closed` distinguishes an auto-closed
  // `(…)` (hide once at/after the `)`) from a still-unclosed `(…` being typed.
  const closed = state.doc.sliceString(argList.to - 1, argList.to) === ")";
  if (head <= argList.from) return null;
  if (closed ? head >= argList.to : head > argList.to) return null;

  const call = argList.parent;
  if (!call) return null;
  const nameNode =
    call.name === "ModuleInstantiation"
      ? call.getChild("ModuleName")
      : call.name === "CallExpression"
        ? call.getChild("VariableName")
        : null;
  if (!nameNode) return null;

  const name = state.doc.sliceString(nameNode.from, nameNode.to);
  const b = BUILTIN_BY_NAME.get(name);
  if (b) return { name, signature: b.signature, doc: b.doc };
  return findUserSignature(state, name);
}

/** Build the tooltip for the current cursor, or null when not inside a call. */
function signatureTooltip(state: EditorState): Tooltip | null {
  const info = signatureAt(state);
  if (!info) return null;
  return {
    pos: state.selection.main.head,
    above: true, // above the line, clear of the text and the completion list
    create() {
      const dom = document.createElement("div");
      dom.className = "cm-signature-help";
      const sig = document.createElement("div");
      sig.className = "cm-signature-sig";
      sig.textContent = info.signature;
      dom.appendChild(sig);
      if (info.doc) {
        const doc = document.createElement("div");
        doc.className = "cm-signature-doc";
        doc.textContent = info.doc;
        dom.appendChild(doc);
      }
      return { dom };
    },
  };
}

const signatureField = StateField.define<Tooltip | null>({
  create: signatureTooltip,
  update(value, tr) {
    if (!tr.docChanged && !tr.selection) return value;
    return signatureTooltip(tr.state);
  },
  provide: (f) => showTooltip.from(f),
});

// Structure only; background/border/text color are inherited from CodeMirror's
// built-in .cm-tooltip defaults, which are theme-aware via the { dark } flag
// theme.ts passes — so light/dark follows automatically.
const signatureTheme = EditorView.baseTheme({
  ".cm-signature-help": { padding: "4px 8px", maxWidth: "40em" },
  ".cm-signature-sig": {
    fontFamily: "ui-monospace, Menlo, monospace",
    whiteSpace: "pre-wrap",
  },
  ".cm-signature-doc": {
    marginTop: "3px",
    opacity: "0.75",
    fontStyle: "italic",
  },
});

/** The signature-help extension: a cursor-driven tooltip + its structural CSS. */
export function signatureHelp(): Extension {
  return [signatureField, signatureTheme];
}
