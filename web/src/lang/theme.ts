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
    // #008000 was 4.49:1 on the active-line bg — a hair under 4.5. Darkened.
    color: "#007000",
    fontStyle: "italic",
  },
  { tag: [t.operator, t.definitionOperator], color: "#000000" },
]);

// Editor chrome (background, caret, gutters, active line, brackets, tooltip) is
// aligned to the app's design tokens so the editor doesn't read as a separate
// VSCode pane bolted into the app — the seam disappears. The *syntax* colors
// above are deliberately left as VSCode Dark+/Light+ (muscle memory). `{ dark }`
// still tells CodeMirror which shade to draw its own bits (scrollbars) in.
// The selection wash keeps a VSCode-style blue: the app's one accent is amber
// (= active/clickable) and its cyan is the viewport's "pointed-at", so neither
// should double as a text selection.

const chrome = (selection: string) => ({
  "&": {
    height: "100%",
    fontSize: "13px",
    color: "var(--text)",
    backgroundColor: "var(--bg)",
  },
  ".cm-scroller": { fontFamily: "var(--font-mono)" },
  ".cm-content": { caretColor: "var(--text)" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--text)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
    {
      backgroundColor: selection,
    },
  ".cm-gutters": {
    backgroundColor: "var(--bg)",
    color: "var(--muted)",
    border: "none",
  },
  ".cm-activeLine": { backgroundColor: "var(--panel)" },
  ".cm-activeLineGutter": { backgroundColor: "var(--panel)" },
  ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
    backgroundColor: "var(--raised)",
    outline: "1px solid var(--muted)",
  },
  ".cm-tooltip-autocomplete": {
    backgroundColor: "var(--raised)",
    border: "1px solid var(--border)",
  },
  ".cm-tooltip-autocomplete > ul > li": { color: "var(--text)" },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
    backgroundColor: "var(--accent)",
    color: "var(--bg)",
  },
  ".cm-completionLabel": { color: "inherit" },
  ".cm-completionDetail": { color: "var(--muted)", fontStyle: "italic" },
  ".cm-tooltip.cm-completionInfo": {
    backgroundColor: "var(--raised)",
    border: "1px solid var(--border)",
    color: "var(--text)",
  },
});

export const darkTheme = EditorView.theme(chrome("#264F78"), { dark: true });
export const lightTheme = EditorView.theme(chrome("#ADD6FF"), { dark: false });
