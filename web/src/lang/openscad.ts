// CodeMirror language support for OpenSCAD, backed by the Lezer grammar in
// openscad.grammar (compiled at build time by the @lezer/generator Vite plugin).
import {
  LRLanguage,
  LanguageSupport,
  foldNodeProp,
  foldInside,
  indentNodeProp,
  delimitedIndent,
} from "@codemirror/language";
import { styleTags, tags as t } from "@lezer/highlight";
import { parser } from "./openscad.grammar";
import { openscadCompletion } from "./complete";
import { signatureHelp } from "./signature";

const parserWithMetadata = parser.configure({
  props: [
    styleTags({
      "if else for each": t.controlKeyword,
      "module function let": t.definitionKeyword,
      "include use": t.moduleKeyword,
      IncludePath: t.string,
      "true false": t.bool,
      undef: t.null,
      Number: t.number,
      String: t.string,
      LineComment: t.lineComment,
      BlockComment: t.blockComment,
      SpecialVariableName: t.special(t.variableName),
      ModuleName: t.function(t.variableName),
      "CallExpression/VariableName": t.function(t.variableName),
      "ModuleDefinition/VariableName": t.function(t.definition(t.variableName)),
      "FunctionDefinition/VariableName": t.function(t.definition(t.variableName)),
      VariableName: t.variableName,
      Modifier: t.modifier,
      '"(" ")"': t.paren,
      '"[" "]"': t.squareBracket,
      '"{" "}"': t.brace,
      '"="': t.definitionOperator,
      '"+" "-" "*" "/" "%" "^" "<" "<=" ">" ">=" "==" "!=" "&&" "||" "!" "?" ":"':
        t.operator,
      '";" ","': t.separator,
    }),
    foldNodeProp.add({
      Block: foldInside,
      Vector: foldInside,
    }),
    indentNodeProp.add({
      Block: delimitedIndent({ closing: "}" }),
    }),
  ],
});

export const openscadLanguage = LRLanguage.define({
  name: "openscad",
  parser: parserWithMetadata,
  languageData: {
    commentTokens: { line: "//", block: { open: "/*", close: "*/" } },
    closeBrackets: { brackets: ["(", "[", "{", '"'] },
    indentOnInput: /^\s*\}$/,
    autocomplete: openscadCompletion,
  },
});

export function openscad(): LanguageSupport {
  return new LanguageSupport(openscadLanguage, [signatureHelp()]);
}
