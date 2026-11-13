import { existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runTests } from "@vscode/test-electron";

const aqui = dirname(fileURLToPath(import.meta.url));
// Usa um VS Code instalado se houver (QJO_VSCODE ou o do macOS); senão o test-electron baixa um.
const instalado = process.env.QJO_VSCODE ?? "/Applications/Visual Studio Code.app/Contents/MacOS/Code";
const perfil = mkdtempSync(join(tmpdir(), "qjo-vscode-"));
await runTests({
  ...(existsSync(instalado) ? { vscodeExecutablePath: instalado } : {}),
  extensionDevelopmentPath: resolve(aqui, ".."),
  extensionTestsPath: resolve(aqui, "suite.cjs"),
  launchArgs: [
    resolve(aqui, "..", "..", "..", "examples"),
    "--disable-extensions",
    "--disable-workspace-trust",
    "--user-data-dir", join(perfil, "dados"),
    "--extensions-dir", join(perfil, "extensoes"),
  ],
}).catch((e) => {
  console.error(e);
  process.exit(1);
});
