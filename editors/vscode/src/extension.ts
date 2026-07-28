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
import { PreviewPanel, PreviewNotification } from "./preview";

let client: LanguageClient | undefined;

/** URI (as a string) of the document currently mirrored in the preview panel. */
let previewUri: string | undefined;

/**
 * Locate a Quito binary: an explicit setting wins, otherwise probe the
 * workspace's `target/{release,debug}` directories (a `cargo build` output),
 * finally fall back to the bare name (resolved via PATH).
 */
function findBinary(configKey: string, baseName: string): string {
  const configured = vscode.workspace.getConfiguration("quito").get<string>(configKey);
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
  const command = findBinary("lsp.path", "quito-lsp");
  const serverOptions: ServerOptions = {
    run: { command, transport: TransportKind.stdio },
    debug: { command, transport: TransportKind.stdio },
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "openscad" }],
    outputChannelName: "Quito Language Server",
  };
  const c = new LanguageClient("quito", "Quito OpenSCAD", serverOptions, clientOptions);

  // The server pushes rendered geometry for previewed documents; route it to the
  // panel when it's the document we're showing.
  c.onNotification("quito/preview", (params: PreviewNotification) => {
    if (params?.uri === previewUri) {
      PreviewPanel.current?.push(params);
    }
  });

  c.start().catch((err) => {
    vscode.window.showErrorMessage(
      `Quito language server failed to start (${command}). Build it with \`cargo build --release -p quito-lsp\` or set quito.lsp.path. ${err}`
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
  return vscode.workspace.getConfiguration("quito").get<boolean>("preview.autoRefresh", true);
}

function startPreview(uri: string): void {
  void serverCommand("quito.startPreview", [uri, { live: previewIsLive() }]);
}

function stopPreview(uri: string): void {
  void serverCommand("quito.stopPreview", [uri]);
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
  startPreview(uri);
}

/** Export the active document via the `quito` CLI, format chosen interactively. */
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
  const cli = findBinary("cli.path", "quito");

  await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: `Quito: exporting ${path.basename(output)}…` },
    () =>
      new Promise<void>((resolve) => {
        const proc = spawn(cli, [input, "-o", output]);
        let stderr = "";
        proc.stderr.on("data", (d) => (stderr += d.toString()));
        proc.on("error", (err) => {
          vscode.window.showErrorMessage(
            `Quito CLI not found (${cli}). Build it with \`cargo build --release -p quito-cli\` or set quito.cli.path. ${err.message}`
          );
          resolve();
        });
        proc.on("close", (code) => {
          if (code === 0) {
            vscode.window.showInformationMessage(`Quito: wrote ${output}`);
          } else {
            vscode.window.showErrorMessage(`Quito export failed: ${stderr.trim() || `exit ${code}`}`);
          }
          resolve();
        });
      })
  );
}

export function activate(context: vscode.ExtensionContext): void {
  client = startClient(context);

  context.subscriptions.push(
    vscode.commands.registerCommand("quito.openPreview", () => {
      PreviewPanel.createOrShow(context, {
        // The webview is up; render whatever's active now.
        onReady: () => switchPreviewTo(vscode.window.activeTextEditor, true),
        onDispose: () => {
          if (previewUri) {
            stopPreview(previewUri);
            previewUri = undefined;
          }
        },
      });
    }),
    vscode.commands.registerCommand("quito.export", () => exportModel()),
    vscode.commands.registerCommand("quito.restartServer", async () => {
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
    // Live-vs-save-only toggled: re-register the current preview with the flag.
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("quito.preview.autoRefresh") && previewUri) {
        startPreview(previewUri);
      }
    })
  );
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
