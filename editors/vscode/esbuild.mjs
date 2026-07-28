// Bundles the two halves of the extension:
//   * the extension host (Node/CommonJS, `vscode` external)  -> dist/extension.js
//   * the webview viewer (browser/ESM, bundles three.js)     -> media/webview.js
//
// The webview is a pure display surface: geometry is computed by the quito-lsp
// server and pushed in via postMessage, so there's no wasm engine to bundle.

import * as esbuild from "esbuild";

const watch = process.argv.includes("--watch");

/** @type {import('esbuild').BuildOptions} */
const host = {
  entryPoints: ["src/extension.ts"],
  outfile: "dist/extension.js",
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node18",
  external: ["vscode"],
  sourcemap: true,
  logLevel: "info",
};

/** @type {import('esbuild').BuildOptions} */
const webview = {
  entryPoints: ["src/webview/main.ts"],
  outfile: "media/webview.js",
  bundle: true,
  platform: "browser",
  format: "esm",
  target: "es2021",
  sourcemap: true,
  logLevel: "info",
};

if (watch) {
  const c1 = await esbuild.context(host);
  const c2 = await esbuild.context(webview);
  await Promise.all([c1.watch(), c2.watch()]);
  console.log("esbuild watching…");
} else {
  await Promise.all([esbuild.build(host), esbuild.build(webview)]);
}
