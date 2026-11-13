// Roda dentro de um VS Code de verdade, com o qjo-lsp embutido em server/.
const vscode = require("vscode");
const assert = require("assert");
const path = require("path");
const fs = require("fs");

const espera = (ms) => new Promise((r) => setTimeout(r, ms));
async function ate(cond, msg) {
  for (let i = 0; i < 100; i++) {
    const v = await cond();
    if (v) return v;
    await espera(100);
  }
  throw new Error("tempo esgotado: " + msg);
}
const ok = (n) => console.log("  ✓ " + n);

exports.run = async function () {
  const ext = vscode.extensions.getExtension("yuhtin.qjo");
  const exemplo = path.resolve(__dirname, "..", "..", "..", "examples", "pegar_roupa.qjo");
  const doc = await vscode.workspace.openTextDocument(exemplo);
  await vscode.window.showTextDocument(doc);
  await ext.activate();
  assert.strictEqual(doc.languageId, "qjo");
  ok(".qjo abre como Q?JO!");

  const ruim = await vscode.workspace.openTextDocument({ language: "qjo", content: fs.readFileSync(exemplo, "utf8").replace("1min);\n        PARAR_INDICACAO(camisa)", "1mn);\n        PARAR_INDICACAO(camisa)") });
  await vscode.window.showTextDocument(ruim);
  const d = await ate(() => { const x = vscode.languages.getDiagnostics(ruim.uri); return x.length ? x : null; }, "diagnóstico");
  assert.strictEqual(d[0].code, "E_LEX_LITERAL");
  ok("erro do servidor: " + d[0].message);
  const acoes = await vscode.commands.executeCommand("vscode.executeCodeActionProvider", ruim.uri, d[0].range);
  const fx = acoes.find((a) => a.kind && a.kind.value === "quickfix");
  await vscode.workspace.applyEdit(fx.edit);
  await ate(() => vscode.languages.getDiagnostics(ruim.uri).length === 0, "correção");
  ok("correção rápida: " + fx.title);

  await vscode.window.showTextDocument(doc);
  const linha = doc.getText().split("\n").findIndex((l) => l.includes("INDICAR(camisa)"));
  const hv = await ate(async () => { const h = await vscode.commands.executeCommand("vscode.executeHoverProvider", doc.uri, new vscode.Position(linha, 10)); return h.length ? h : null; }, "hover");
  assert.ok(hv[0].contents.map((c) => c.value || c).join(" ").includes("INDICAR(objeto)"));
  ok("dica");
  const def = await vscode.commands.executeCommand("vscode.executeDefinitionProvider", doc.uri, new vscode.Position(linha, 17));
  assert.strictEqual(def[0].range.start.line, 3);
  ok("ir para a definição");
  const comp = await vscode.commands.executeCommand("vscode.executeCompletionItemProvider", doc.uri, new vscode.Position(linha, 8));
  const rot = comp.items.map((i) => (typeof i.label === "string" ? i.label : i.label.label));
  assert.ok(rot.includes("camisa") && rot.includes("AGUARDAR_CONFIRMACAO"));
  ok("autocompletar");
  const sim = await vscode.commands.executeCommand("vscode.executeDocumentSymbolProvider", doc.uri);
  assert.ok(sim.some((s) => s.name === "Pegar a roupa" && s.children.length === 2));
  ok("esboço com rotina e etapas");
  const ren = await vscode.commands.executeCommand("vscode.executeDocumentRenameProvider", doc.uri, new vscode.Position(3, 9), "camiseta");
  assert.strictEqual(ren.get(doc.uri).length, 3);
  ok("renomear");

  const feio = await vscode.workspace.openTextDocument({ language: "qjo", content: 'data h=data"08:00:00";rotina("R",h){ESPERAR(1s);}' });
  const fmt = await vscode.commands.executeCommand("vscode.executeFormatDocumentProvider", feio.uri, { tabSize: 4, insertSpaces: true });
  const we = new vscode.WorkspaceEdit();
  fmt.forEach((e) => we.replace(feio.uri, e.range, e.newText));
  await vscode.workspace.applyEdit(we);
  assert.strictEqual(feio.getText(), 'data h = data"08:00:00";\nrotina("R", h) {\n    ESPERAR(1s);\n}\n');
  ok("formatar");

  const cmds = await vscode.commands.getCommands(true);
  ["qjo.rodar", "qjo.cpp", "qjo.reiniciar"].forEach((c) => assert.ok(cmds.includes(c), c));
  await vscode.window.showTextDocument(doc);
  const cpp = exemplo.replace(/\.qjo$/, ".cpp");
  await vscode.commands.executeCommand("qjo.cpp");
  await ate(() => fs.existsSync(cpp), "C++ gerado");
  assert.ok(fs.readFileSync(cpp, "utf8").includes("int main()"));
  fs.unlinkSync(cpp);
  ok("gerar C++ pela CLI embutida");
  console.log("\nTudo certo.");
};
