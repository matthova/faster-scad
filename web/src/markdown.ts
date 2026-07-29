import { marked } from "marked";

/** Render markdown to HTML. All links get target="_blank" and rel="noopener noreferrer". */
export function renderMarkdown(md: string): string {
  const html = marked.parse(md, { async: false }) as string;
  return html.replace(/<a\s/g, '<a target="_blank" rel="noopener noreferrer" ');
}
