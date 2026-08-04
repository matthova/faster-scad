// Editor theming for the OpenSCAD CodeMirror editor: VSCode Dark+/Light+-style
// syntax colors plus matching editor chrome (background, caret, selection,
// gutters, autocomplete tooltip). Both a dark and a light variant are exported;
// App.tsx swaps between them in a Compartment as the OS appearance changes.
import { EditorView } from "@codemirror/view";
import { HighlightStyle } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

// Syntax colors, keyed on the @lezer/highlight tags emitted by openscad.ts's
// styleTags. `t.function(t.variableName)` matches both call/module names and
// `t.function(t.definition(...))` via tag inheritance, so one entry covers all
// three. The colors mirror VSCode's Dark+ / Light+ themes.

export const darkHighlight = HighlightStyle.define([
  {
    tag: [t.definitionKeyword, t.moduleKeyword, t.bool, t.null, t.modifier],
    color: "#569CD6",
  },
  { tag: t.controlKeyword, color: "#C586C0" },
  { tag: t.function(t.variableName), color: "#DCDCAA" },
  { tag: t.special(t.variableName), color: "#4EC9B0" },
  { tag: t.variableName, color: "#9CDCFE" },
  { tag: t.string, color: "#CE9178" },
  { tag: t.number, color: "#B5CEA8" },
  {
    tag: [t.lineComment, t.blockComment],
    color: "#6A9955",
    fontStyle: "italic",
  },
  { tag: [t.operator, t.definitionOperator], color: "#D4D4D4" },
]);

export const lightHighlight = HighlightStyle.define([
  {
    tag: [t.definitionKeyword, t.moduleKeyword, t.bool, t.null, t.modifier],
    color: "#0000FF",
  },
  { tag: t.controlKeyword, color: "#AF00DB" },
  { tag: t.function(t.variableName), color: "#795E26" },
  { tag: t.special(t.variableName), color: "#267F99" },
  { tag: t.variableName, color: "#001080" },
  { tag: t.string, color: "#A31515" },
  { tag: t.number, color: "#098658" },
  {
    tag: [t.lineComment, t.blockComment],
    color: "#008000",
    fontStyle: "italic",
  },
  { tag: [t.operator, t.definitionOperator], color: "#000000" },
]);

// Editor chrome: root color/background, font, caret, selection, gutters, active
// line, matching brackets, and the autocomplete tooltip. `{ dark }` lets
// CodeMirror pick the right shade for the bits it draws itself (scrollbars,
// panels). Colors are VSCode's editor defaults.

export const darkTheme = EditorView.theme(
  {
    "&": {
      height: "100%",
      fontSize: "13px",
      color: "#D4D4D4",
      backgroundColor: "#1E1E1E",
    },
    ".cm-scroller": { fontFamily: "ui-monospace, Menlo, monospace" },
    ".cm-content": { caretColor: "#AEAFAD" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "#AEAFAD" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
      {
        backgroundColor: "#264F78",
      },
    ".cm-gutters": {
      backgroundColor: "#1E1E1E",
      color: "#858585",
      border: "none",
    },
    ".cm-activeLine": { backgroundColor: "#2A2A2A" },
    ".cm-activeLineGutter": { backgroundColor: "#2A2A2A" },
    ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
      backgroundColor: "#3A3D41",
      outline: "1px solid #888",
    },
    // Autocomplete tooltip, styled to feel native.
    ".cm-tooltip-autocomplete": {
      backgroundColor: "#252526",
      border: "1px solid #454545",
    },
    ".cm-tooltip-autocomplete > ul > li": { color: "#D4D4D4" },
    ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
      backgroundColor: "#04395E",
      color: "#FFFFFF",
    },
    ".cm-completionLabel": { color: "inherit" },
    ".cm-completionDetail": { color: "#9AA0AA", fontStyle: "italic" },
    ".cm-tooltip.cm-completionInfo": {
      backgroundColor: "#252526",
      border: "1px solid #454545",
      color: "#D4D4D4",
    },
  },
  { dark: true },
);

export const lightTheme = EditorView.theme(
  {
    "&": {
      height: "100%",
      fontSize: "13px",
      color: "#000000",
      backgroundColor: "#FFFFFF",
    },
    ".cm-scroller": { fontFamily: "ui-monospace, Menlo, monospace" },
    ".cm-content": { caretColor: "#000000" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "#000000" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
      {
        backgroundColor: "#ADD6FF",
      },
    ".cm-gutters": {
      backgroundColor: "#F3F3F3",
      color: "#858585",
      border: "none",
    },
    ".cm-activeLine": { backgroundColor: "#F5F5F5" },
    ".cm-activeLineGutter": { backgroundColor: "#F5F5F5" },
    ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
      backgroundColor: "#DDDDDD",
      outline: "1px solid #B9B9B9",
    },
    ".cm-tooltip-autocomplete": {
      backgroundColor: "#F3F3F3",
      border: "1px solid #C8C8C8",
    },
    ".cm-tooltip-autocomplete > ul > li": { color: "#000000" },
    ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
      backgroundColor: "#0060C0",
      color: "#FFFFFF",
    },
    ".cm-completionLabel": { color: "inherit" },
    ".cm-completionDetail": { color: "#6B6B6B", fontStyle: "italic" },
    ".cm-tooltip.cm-completionInfo": {
      backgroundColor: "#F3F3F3",
      border: "1px solid #C8C8C8",
      color: "#000000",
    },
  },
  { dark: false },
);
