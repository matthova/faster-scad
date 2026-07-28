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
import { PreviewPanel } from "./preview";

let client: LanguageClient | undefined;

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
  c.start().catch((err) => {
    vscode.window.showErrorMessage(
      `Quito language server failed to start (${command}). Build it with \`cargo build --release -p quito-lsp\` or set quito.lsp.path. ${err}`
    );
  });
  return c;
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
      PreviewPanel.createOrShow(context);
    }),
    vscode.commands.registerCommand("quito.export", () => exportModel()),
    vscode.commands.registerCommand("quito.restartServer", async () => {
      await client?.stop();
      client = startClient(context);
    })
  );

  // Drive the live preview from editor activity.
  let debounce: ReturnType<typeof setTimeout> | undefined;
  const refresh = (doc: vscode.TextDocument, immediate: boolean) => {
    if (doc.languageId !== "openscad" || !PreviewPanel.current) {
      return;
    }
    const push = () => PreviewPanel.current?.update(doc);
    if (immediate) {
      push();
      return;
    }
    if (debounce) {
      clearTimeout(debounce);
    }
    debounce = setTimeout(push, 250);
  };

  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((e) => {
      const auto = vscode.workspace.getConfiguration("quito").get<boolean>("preview.autoRefresh", true);
      if (auto) {
        refresh(e.document, false);
      }
    }),
    vscode.workspace.onDidSaveTextDocument((doc) => refresh(doc, true)),
    vscode.window.onDidChangeActiveTextEditor((ed) => {
      if (ed) {
        refresh(ed.document, true);
      }
    })
  );
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
