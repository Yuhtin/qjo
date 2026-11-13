// Extensão Q?JO!: cliente do qjo-lsp (todo o entendimento da linguagem fica no servidor)
// e dois comandos que chamam a CLI qjo: rodar a rotina no terminal e gerar C++.
import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from "vscode-languageclient/node";

let cliente: LanguageClient | undefined;

function exe(nome: string): string {
  return process.platform === "win32" ? `${nome}.exe` : nome;
}

/** Configuração > binário que vem na extensão > PATH. */
function binario(ctx: vscode.ExtensionContext, chave: string, nome: string): string {
  const configurado = vscode.workspace.getConfiguration("qjo").get<string>(chave, "").trim();
  if (configurado) return configurado;
  const embutido = ctx.asAbsolutePath(path.join("server", exe(nome)));
  if (fs.existsSync(embutido)) return embutido;
  return nome;
}

async function iniciar(ctx: vscode.ExtensionContext): Promise<void> {
  const comando = binario(ctx, "servidor", "qjo-lsp");
  const servidor: ServerOptions = { command: comando, transport: TransportKind.stdio };
  const opcoes: LanguageClientOptions = {
    documentSelector: [{ language: "qjo" }],
    outputChannelName: "Q?JO!",
  };
  cliente = new LanguageClient("qjo", "Q?JO!", servidor, opcoes);
  try {
    await cliente.start();
  } catch (e) {
    cliente = undefined;
    const escolha = await vscode.window.showErrorMessage(
      `Não consegui iniciar o servidor da Q?JO! (${comando}). Instale o qjo-lsp ou aponte o caminho em "qjo.servidor".`,
      "Abrir configuração",
    );
    if (escolha) void vscode.commands.executeCommand("workbench.action.openSettings", "qjo.servidor");
  }
}

function arquivoAtivo(): vscode.TextDocument | undefined {
  const doc = vscode.window.activeTextEditor?.document;
  if (!doc || doc.languageId !== "qjo") {
    void vscode.window.showWarningMessage("Abra um arquivo .qjo primeiro.");
    return undefined;
  }
  return doc;
}

async function salvo(doc: vscode.TextDocument): Promise<string | undefined> {
  if (doc.isUntitled) {
    void vscode.window.showWarningMessage("Salve o arquivo antes (o qjo trabalha com o arquivo no disco).");
    return undefined;
  }
  if (doc.isDirty) await doc.save();
  return doc.fileName;
}

function aspas(s: string): string {
  return process.platform === "win32" ? `"${s}"` : `'${s.replace(/'/g, `'\\''`)}'`;
}

export async function activate(ctx: vscode.ExtensionContext): Promise<{ cliente: () => LanguageClient | undefined }> {
  ctx.subscriptions.push(
    vscode.commands.registerCommand("qjo.rodar", async () => {
      const doc = arquivoAtivo();
      const f = doc && (await salvo(doc));
      if (!f) return;
      const term = vscode.window.createTerminal({ name: `Q?JO! ${path.basename(f)}`, cwd: path.dirname(f) });
      term.show();
      term.sendText(`${aspas(binario(ctx, "cli", "qjo"))} run ${aspas(f)}`);
    }),
    vscode.commands.registerCommand("qjo.cpp", async () => {
      const doc = arquivoAtivo();
      const f = doc && (await salvo(doc));
      if (!f) return;
      const saida = f.replace(/\.qjo$/i, "") + ".cpp";
      const { execFile } = await import("node:child_process");
      execFile(binario(ctx, "cli", "qjo"), ["build", f, "-o", saida], async (erro, _out, err) => {
        if (erro) {
          void vscode.window.showErrorMessage(err.split("\n").find((l) => l.trim()) ?? String(erro));
          return;
        }
        const cpp = await vscode.workspace.openTextDocument(saida);
        await vscode.window.showTextDocument(cpp, { viewColumn: vscode.ViewColumn.Beside, preview: true });
      });
    }),
    vscode.commands.registerCommand("qjo.reiniciar", async () => {
      await cliente?.stop();
      await iniciar(ctx);
    }),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("qjo.servidor")) void vscode.commands.executeCommand("qjo.reiniciar");
    }),
  );
  await iniciar(ctx);
  return { cliente: () => cliente };
}

export async function deactivate(): Promise<void> {
  await cliente?.stop();
}
