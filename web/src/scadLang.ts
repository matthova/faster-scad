// A lightweight OpenSCAD syntax highlighter using CodeMirror's StreamLanguage.
// (A full Lezer grammar is a later milestone; this covers keywords, builtins,
// comments, strings, numbers, and special `$` variables.)
import { StreamLanguage, StringStream } from "@codemirror/language";

const KEYWORDS = new Set([
  "module",
  "function",
  "if",
  "else",
  "for",
  "let",
  "true",
  "false",
  "undef",
  "each",
  "echo",
  "assert",
  "include",
  "use",
]);

const BUILTINS = new Set([
  "cube",
  "sphere",
  "cylinder",
  "polyhedron",
  "polygon",
  "square",
  "circle",
  "text",
  "translate",
  "rotate",
  "scale",
  "mirror",
  "resize",
  "multmatrix",
  "color",
  "union",
  "difference",
  "intersection",
  "hull",
  "minkowski",
  "linear_extrude",
  "rotate_extrude",
  "offset",
  "projection",
  "children",
  "render",
]);

interface ScadState {
  inComment: boolean;
}

export const scad = StreamLanguage.define<ScadState>({
  startState: () => ({ inComment: false }),
  token(stream: StringStream, state: ScadState): string | null {
    if (state.inComment) {
      if (stream.match(/.*?\*\//)) {
        state.inComment = false;
      } else {
        stream.skipToEnd();
      }
      return "comment";
    }
    if (stream.match("/*")) {
      state.inComment = true;
      return "comment";
    }
    if (stream.match(/\/\/.*/)) return "comment";
    if (stream.match(/"(?:[^"\\]|\\.)*"/)) return "string";
    if (stream.match(/(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:[eE][+-]?[0-9]+)?/))
      return "number";
    if (stream.match(/\$[A-Za-z_][A-Za-z0-9_]*/)) return "variableName.special";
    if (stream.match(/[A-Za-z_][A-Za-z0-9_]*/)) {
      const word = stream.current();
      if (KEYWORDS.has(word)) return "keyword";
      if (BUILTINS.has(word)) return "typeName";
      return "variableName";
    }
    if (stream.match(/[+\-*/%<>=!&|?:]+/)) return "operator";
    stream.next();
    return null;
  },
  tokenTable: {},
});
