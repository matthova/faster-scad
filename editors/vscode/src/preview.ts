import * as fs from "fs";
import * as vscode from "vscode";

/**
 * The live 3D preview. A single reused webview panel hosts a three.js viewer
 * and the Quito wasm engine (built into `media/engine`). The extension streams
 * document source in; the webview renders and returns errors/stats.
 */
export class PreviewPanel {
  static current: PreviewPanel | undefined;

  private readonly panel: vscode.WebviewPanel;
  private readonly extensionUri: vscode.Uri;
  private ready = false;
  /** Source received before the webview finished booting, replayed on ready. */
  private pending: string | undefined;
  private disposables: vscode.Disposable[] = [];

  static createOrShow(context: vscode.ExtensionContext): void {
    const column = vscode.ViewColumn.Beside;
    if (PreviewPanel.current) {
      PreviewPanel.current.panel.reveal(column);
      PreviewPanel.current.pushActive();
      return;
    }
    const engineDir = vscode.Uri.joinPath(context.extensionUri, "media", "engine");
    if (!fs.existsSync(vscode.Uri.joinPath(engineDir, "quito.js").fsPath)) {
      vscode.window.showErrorMessage(
        "Quito preview engine is missing. Build it with `npm run build:wasm` in editors/vscode."
      );
      return;
    }
    const panel = vscode.window.createWebviewPanel(
      "quitoPreview",
      "Quito Preview",
      column,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, "media")],
      }
    );
    PreviewPanel.current = new PreviewPanel(panel, context.extensionUri);
  }

  private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
    this.panel = panel;
    this.extensionUri = extensionUri;
    this.panel.webview.html = this.html();

    this.panel.webview.onDidReceiveMessage(
      (msg) => {
        switch (msg?.type) {
          case "loaded":
            // Hand the webview the engine URIs to dynamically import.
            this.panel.webview.postMessage({
              type: "init",
              engineJs: this.uri("engine", "quito.js"),
              wasmUri: this.uri("engine", "quito_bg.wasm"),
            });
            break;
          case "ready":
            this.ready = true;
            if (this.pending !== undefined) {
              this.render(this.pending);
              this.pending = undefined;
            } else {
              this.pushActive();
            }
            break;
        }
      },
      undefined,
      this.disposables
    );

    this.panel.onDidDispose(() => this.dispose(), undefined, this.disposables);
  }

  /** Render whichever OpenSCAD document is currently active, if any. */
  pushActive(): void {
    const editor = vscode.window.activeTextEditor;
    if (editor?.document.languageId === "openscad") {
      this.update(editor.document);
    }
  }

  /** Send a document's source to the webview for rendering. */
  update(doc: vscode.TextDocument): void {
    this.render(doc.getText());
  }

  private render(source: string): void {
    if (!this.ready) {
      this.pending = source;
      return;
    }
    this.panel.webview.postMessage({ type: "render", source });
  }

  private uri(...parts: string[]): string {
    return this.panel.webview
      .asWebviewUri(vscode.Uri.joinPath(this.extensionUri, "media", ...parts))
      .toString();
  }

  private html(): string {
    const webview = this.panel.webview;
    const scriptUri = this.uri("webview.js");
    // Host-source CSP: scripts (incl. the dynamically-imported engine glue) come
    // from the webview origin; wasm needs 'wasm-unsafe-eval'; the .wasm is
    // fetched by URL, hence connect-src.
    const csp = [
      `default-src 'none'`,
      `img-src ${webview.cspSource}`,
      `style-src ${webview.cspSource} 'unsafe-inline'`,
      `script-src ${webview.cspSource} 'wasm-unsafe-eval'`,
      `connect-src ${webview.cspSource}`,
    ].join("; ");

    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <style>
    html, body { margin: 0; height: 100%; overflow: hidden; background: #1e1e1e; }
    #canvas { width: 100vw; height: 100vh; display: block; }
    #status {
      position: fixed; left: 8px; bottom: 8px; right: 8px;
      font: 12px/1.4 var(--vscode-editor-font-family, monospace);
      color: var(--vscode-foreground, #ccc); white-space: pre-wrap; pointer-events: none;
    }
    #status.error { color: var(--vscode-errorForeground, #f48771); }
  </style>
</head>
<body>
  <canvas id="canvas"></canvas>
  <div id="status">Loading engine…</div>
  <script type="module" src="${scriptUri}"></script>
</body>
</html>`;
  }

  private dispose(): void {
    PreviewPanel.current = undefined;
    this.panel.dispose();
    for (const d of this.disposables) {
      d.dispose();
    }
    this.disposables = [];
  }
}
