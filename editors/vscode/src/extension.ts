import * as path from "path";
import * as fs from "fs";
import { spawn } from "child_process";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import { PreviewPanel, PreviewNotification, ProvenanceGroup, Span } from "./preview";

let client: LanguageClient | undefined;

/** URI (as a string) of the document currently mirrored in the preview panel. */
let previewUri: string | undefined;

/** Provenance groups for the previewed document (from the latest `openrscad/preview`
 *  notification), used to resolve the editor cursor → source span (code→model). */
let previewProvenance: ProvenanceGroup[] = [];

/** UTF-8 byte length of a Unicode code point. */
function utf8Len(cp: number): number {
  return cp <= 0x7f ? 1 : cp <= 0x7ff ? 2 : cp <= 0xffff ? 3 : 4;
}

/** Map a UTF-8 byte offset (engine spans) to a UTF-16 index (VS Code positions). */
function byteToChar(source: string, byte: number): number {
  if (byte <= 0) return 0;
  let b = 0;
  let i = 0;
  while (i < source.length) {
    if (b >= byte) return i;
    const cp = source.codePointAt(i)!;
    b += utf8Len(cp);
    i += cp > 0xffff ? 2 : 1;
  }
  return source.length;
}

/** Map a UTF-16 index (VS Code positions) to a UTF-8 byte offset (engine spans). */
function charToByte(source: string, char: number): number {
  if (char <= 0) return 0;
  let b = 0;
  let i = 0;
  while (i < source.length && i < char) {
    const cp = source.codePointAt(i)!;
    b += utf8Len(cp);
    i += cp > 0xffff ? 2 : 1;
  }
  return b;
}

/** Model → code: reveal and select the source statement at `span` (byte offsets)
 *  in the previewed document. */
async function revealSpan(span: Span): Promise<void> {
  if (!previewUri) {
    return;
  }
  const doc = await vscode.workspace.openTextDocument(vscode.Uri.parse(previewUri));
  const text = doc.getText();
  const from = doc.positionAt(byteToChar(text, span[0]));
  const to = doc.positionAt(byteToChar(text, span[1]));
  const editor = await vscode.window.showTextDocument(doc, { preview: false });
  editor.selection = new vscode.Selection(from, to);
  editor.revealRange(
    new vscode.Range(from, to),
    vscode.TextEditorRevealType.InCenterIfOutsideViewport
  );
}

/**
 * Locate a OpenRSCAD binary: an explicit setting wins, otherwise probe the
 * workspace's `target/{release,debug}` directories (a `cargo build` output),
 * finally fall back to the bare name (resolved via PATH).
 */
function findBinary(configKey: string, baseName: string): string {
  const configured = vscode.workspace.getConfiguration("openrscad").get<string>(configKey);
  if (configured && configured.trim()) {
    return configured.trim();
  }
  const exe = process.platform === "win32" ? `${baseName}.exe` : baseName;
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    for (const profile of ["release", "debug"]) {
      const candidate = path.join(folder.uri.fsPath, "target", profile, exe);
      if (fs.existsSync(candidate)) {
        return candidate;
      }
    }
  }
  return exe;
}

function startClient(context: vscode.ExtensionContext): LanguageClient {
  const command = findBinary("lsp.path", "openrscad-lsp");
  const serverOptions: ServerOptions = {
    run: { command, transport: TransportKind.stdio },
    debug: { command, transport: TransportKind.stdio },
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "openscad" }],
    outputChannelName: "OpenRSCAD Language Server",
  };
  const c = new LanguageClient("openrscad", "OpenRSCAD OpenSCAD", serverOptions, clientOptions);

  // The server pushes rendered geometry for previewed documents; route it to the
  // panel when it's the document we're showing.
  c.onNotification("openrscad/preview", (params: PreviewNotification) => {
    if (params?.uri === previewUri) {
      previewProvenance = params.provenance ?? [];
      PreviewPanel.current?.push(params);
    }
  });

  c.start().catch((err) => {
    vscode.window.showErrorMessage(
      `OpenRSCAD language server failed to start (${command}). Build it with \`cargo build --release -p openrscad-lsp\` or set openrscad.lsp.path. ${err}`
    );
  });
  return c;
}

/** Ask the language server to run one of its `workspace/executeCommand` verbs. */
async function serverCommand(command: string, args: unknown[]): Promise<void> {
  try {
    await client?.sendRequest("workspace/executeCommand", { command, arguments: args });
  } catch {
    // The server may be down (e.g. mid-restart); the preview simply won't update.
  }
}

/** Whether the preview should re-render as the user types (vs. on save only). */
function previewIsLive(): boolean {
  return vscode.workspace.getConfiguration("openrscad").get<boolean>("preview.autoRefresh", true);
}

function startPreview(uri: string): void {
  void serverCommand("openrscad.startPreview", [uri, { live: previewIsLive() }]);
}

function stopPreview(uri: string): void {
  void serverCommand("openrscad.stopPreview", [uri]);
}

/**
 * Point the preview at `editor`'s document (if it's OpenSCAD), stopping the
 * previous one. `force` re-starts even if the URI is unchanged (used when the
 * webview (re)boots or the live setting changes).
 */
function switchPreviewTo(editor: vscode.TextEditor | undefined, force = false): void {
  if (!PreviewPanel.current) {
    return;
  }
  if (editor?.document.languageId !== "openscad") {
    return;
  }
  const uri = editor.document.uri.toString();
  if (uri === previewUri && !force) {
    return;
  }
  if (previewUri && previewUri !== uri) {
    stopPreview(previewUri);
  }
  previewUri = uri;
  previewProvenance = []; // stale until the new document's preview arrives
  PreviewPanel.current?.highlight(null);
  startPreview(uri);
}

/** Export the active document via the `openrscad` CLI, format chosen interactively. */
async function exportModel(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "openscad") {
    vscode.window.showWarningMessage("Open a .scad file to export.");
    return;
  }
  const input = editor.document.uri.fsPath;
  const pick = await vscode.window.showQuickPick(
    [
      { label: "STL (binary)", ext: "stl" },
      { label: "3MF", ext: "3mf" },
      { label: "OBJ", ext: "obj" },
      { label: "OFF", ext: "off" },
      { label: "AMF", ext: "amf" },
      { label: "DXF (2D)", ext: "dxf" },
      { label: "SVG (2D)", ext: "svg" },
      { label: "PNG (rendered image)", ext: "png" },
    ],
    { placeHolder: "Export format" }
  );
  if (!pick) {
    return;
  }
  const output = input.replace(/\.scad$/i, "") + "." + pick.ext;
  await editor.document.save();
  const cli = findBinary("cli.path", "openrscad");

  await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: `OpenRSCAD: exporting ${path.basename(output)}…` },
    () =>
      new Promise<void>((resolve) => {
        const proc = spawn(cli, [input, "-o", output]);
        let stderr = "";
        proc.stderr.on("data", (d) => (stderr += d.toString()));
        proc.on("error", (err) => {
          vscode.window.showErrorMessage(
            `OpenRSCAD CLI not found (${cli}). Build it with \`cargo build --release -p openrscad-cli\` or set openrscad.cli.path. ${err.message}`
          );
          resolve();
        });
        proc.on("close", (code) => {
          if (code === 0) {
            vscode.window.showInformationMessage(`OpenRSCAD: wrote ${output}`);
          } else {
            vscode.window.showErrorMessage(`OpenRSCAD export failed: ${stderr.trim() || `exit ${code}`}`);
          }
          resolve();
        });
      })
  );
}

export function activate(context: vscode.ExtensionContext): void {
  client = startClient(context);

  context.subscriptions.push(
    vscode.commands.registerCommand("openrscad.openPreview", () => {
      PreviewPanel.createOrShow(context, {
        // The webview is up; render whatever's active now.
        onReady: () => switchPreviewTo(vscode.window.activeTextEditor, true),
        onDispose: () => {
          if (previewUri) {
            stopPreview(previewUri);
            previewUri = undefined;
          }
          previewProvenance = [];
        },
        // Model → code: a clicked face selects its source statement; a null pick
        // (clicked empty space, or Escape) deselects the highlighted item.
        onPick: (span) => {
          if (span) {
            void revealSpan(span);
          } else {
            PreviewPanel.current?.highlight(null);
          }
        },
      });
    }),
    vscode.commands.registerCommand("openrscad.export", () => exportModel()),
    vscode.commands.registerCommand("openrscad.restartServer", async () => {
      await client?.stop();
      client = startClient(context);
      // Re-register the live preview with the fresh server.
      if (previewUri) {
        startPreview(previewUri);
      }
    })
  );

  context.subscriptions.push(
    // Follow the active editor: the preview mirrors whichever .scad has focus.
    vscode.window.onDidChangeActiveTextEditor((ed) => switchPreviewTo(ed)),
    // Code → model: highlight the geometry under the cursor as the selection
    // moves in the previewed document.
    vscode.window.onDidChangeTextEditorSelection((e) => {
      if (!PreviewPanel.current || e.textEditor.document.uri.toString() !== previewUri) {
        return;
      }
      const text = e.textEditor.document.getText();
      // Use the selection's start (not its active/head): a model→code click
      // selects the whole clicked statement `[from,to)` and the head lands on the
      // *exclusive* end, which no half-open span contains — so a click would
      // resolve to a parent or nothing. The start byte sits inside the clicked
      // statement, so the click lights exactly that item (code→model parity).
      const byte = charToByte(text, e.textEditor.document.offsetAt(e.selections[0].start));
      // Among every span (at any nesting level) that contains that byte, pick the
      // narrowest — the tightest enclosing statement. highlight() then lights all
      // geometry whose stack contains it (that statement's whole subtree).
      let best: Span | null = null;
      for (const gr of previewProvenance) {
        for (const s of gr.spans) {
          if (byte >= s[0] && byte < s[1] && (!best || s[1] - s[0] < best[1] - best[0])) {
            best = s;
          }
        }
      }
      PreviewPanel.current.highlight(best);
    }),
    // Live-vs-save-only toggled: re-register the current preview with the flag.
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("openrscad.preview.autoRefresh") && previewUri) {
        startPreview(previewUri);
      }
    })
  );
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
