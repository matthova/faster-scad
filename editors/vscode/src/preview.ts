import * as vscode from "vscode";

/** A colored preview group: a vertex range into the soup + its color and mode. */
export interface PreviewGroup {
  start: number;
  count: number;
  color: [number, number, number, number];
  mode: "solid" | "highlight" | "background";
}

/** A source byte-span `[start, end]` into the previewed document. */
export type Span = [number, number];

/** A provenance group: a vertex range into the provenance soup + the stack of
 *  enclosing source spans that produced it (outermost first, innermost last;
 *  empty when unattributable). A click selects the deepest (last) span; the
 *  cursor→model highlight matches any span in the stack by containment. */
export interface ProvenanceGroup {
  start: number;
  count: number;
  spans: Span[];
}

/** The `openrscad/preview` notification payload pushed by the language server. */
export interface PreviewNotification {
  uri: string;
  ok: boolean;
  error?: string;
  positions?: string;
  normals?: string;
  triangleCount?: number;
  vertexCount?: number;
  volume?: number;
  area?: number;
  // Colored channel — present only when the model uses color()/#/%.
  previewPositions?: string;
  previewNormals?: string;
  groups?: PreviewGroup[];
  // Provenance channel for editor↔preview linking — present for 3D models.
  provenancePositions?: string;
  provenanceNormals?: string;
  provenance?: ProvenanceGroup[];
}

/** Callbacks wiring a panel to the server-side preview lifecycle. */
export interface PreviewHooks {
  /** The webview has booted and is ready to receive geometry. */
  onReady: () => void;
  /** The panel was closed. */
  onDispose: () => void;
  /** The user clicked a face in the preview; `span` is the source statement's
   *  byte range (or `null` for empty space). */
  onPick: (span: Span | null) => void;
}

/**
 * The live 3D preview. A single reused webview panel hosts a three.js viewer.
 * Geometry is computed by the openrscad-lsp server (native kernel) and pushed to the
 * extension as `openrscad/preview` notifications; the extension forwards it here via
 * {@link PreviewPanel.push}. The webview itself does no OpenSCAD evaluation.
 */
export class PreviewPanel {
  static current: PreviewPanel | undefined;

  private readonly panel: vscode.WebviewPanel;
  private readonly extensionUri: vscode.Uri;
  private readonly hooks: PreviewHooks;
  private ready = false;
  /** Last notification received before the webview finished booting. */
  private pending: PreviewNotification | undefined;
  private disposables: vscode.Disposable[] = [];

  static createOrShow(context: vscode.ExtensionContext, hooks: PreviewHooks): void {
    const column = vscode.ViewColumn.Beside;
    if (PreviewPanel.current) {
      PreviewPanel.current.panel.reveal(column);
      if (PreviewPanel.current.ready) {
        hooks.onReady();
      }
      return;
    }
    const panel = vscode.window.createWebviewPanel(
      "openrscadPreview",
      "OpenRSCAD Preview",
      column,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, "media")],
      }
    );
    PreviewPanel.current = new PreviewPanel(panel, context.extensionUri, hooks);
  }

  private constructor(
    panel: vscode.WebviewPanel,
    extensionUri: vscode.Uri,
    hooks: PreviewHooks
  ) {
    this.panel = panel;
    this.extensionUri = extensionUri;
    this.hooks = hooks;
    this.panel.webview.html = this.html();

    this.panel.webview.onDidReceiveMessage(
      (msg) => {
        if (msg?.type === "ready") {
          this.ready = true;
          if (this.pending !== undefined) {
            this.push(this.pending);
            this.pending = undefined;
          }
          this.hooks.onReady();
        } else if (msg?.type === "pick") {
          this.hooks.onPick((msg.span as Span | null) ?? null);
        }
      },
      undefined,
      this.disposables
    );

    this.panel.onDidDispose(() => this.dispose(), undefined, this.disposables);
  }

  /** Forward a server `openrscad/preview` notification to the webview. */
  push(note: PreviewNotification): void {
    if (!this.ready) {
      this.pending = note;
      return;
    }
    if (note.ok) {
      this.panel.webview.postMessage({
        type: "mesh",
        positions: note.positions ?? "",
        normals: note.normals ?? "",
        previewPositions: note.previewPositions,
        previewNormals: note.previewNormals,
        groups: note.groups,
        provenancePositions: note.provenancePositions,
        provenanceNormals: note.provenanceNormals,
        provenance: note.provenance,
        triangleCount: note.triangleCount ?? 0,
        vertexCount: note.vertexCount ?? 0,
        volume: note.volume ?? 0,
        area: note.area ?? 0,
      });
    } else {
      this.panel.webview.postMessage({
        type: "error",
        message: note.error ?? "render error",
      });
    }
  }

  /** Highlight the geometry produced by the statement at `span` (code→model), or
   *  clear the highlight when `span` is `null`. Posts to the webview. */
  highlight(span: Span | null): void {
    if (!this.ready) {
      return;
    }
    this.panel.webview.postMessage({ type: "highlight", span });
  }

  private html(): string {
    const webview = this.panel.webview;
    const scriptUri = webview
      .asWebviewUri(vscode.Uri.joinPath(this.extensionUri, "media", "webview.js"))
      .toString();
    // Host-source CSP: the viewer script (with three.js bundled) comes from the
    // webview origin. No wasm and no network fetches — geometry arrives via
    // postMessage from the extension host.
    const csp = [
      `default-src 'none'`,
      `img-src ${webview.cspSource}`,
      `style-src ${webview.cspSource} 'unsafe-inline'`,
      `script-src ${webview.cspSource}`,
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
  <div id="status">Rendering…</div>
  <script type="module" src="${scriptUri}"></script>
</body>
</html>`;
  }

  private dispose(): void {
    PreviewPanel.current = undefined;
    this.hooks.onDispose();
    this.panel.dispose();
    for (const d of this.disposables) {
      d.dispose();
    }
    this.disposables = [];
  }
}
