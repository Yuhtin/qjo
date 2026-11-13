// Coloca qjo-lsp e qjo em server/ para irem dentro do .vsix.
// Sem argumentos: compila para esta máquina. Com QJO_BINARIOS=<pasta>: copia binários já
// compilados (a CI usa isso para gerar um .vsix por plataforma).
import { execFileSync } from "node:child_process";
import { copyFileSync, chmodSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const aqui = dirname(fileURLToPath(import.meta.url));
const raiz = join(aqui, "..", "..", "..");
const destino = join(aqui, "..", "server");
const win = /^win/.test(process.env.QJO_ALVO ?? process.platform);
const exe = (n) => (win ? `${n}.exe` : n);

let origem = process.env.QJO_BINARIOS;
if (!origem) {
  execFileSync("cargo", ["build", "--release", "-p", "qjo-lsp", "-p", "qjo-cli"], { cwd: raiz, stdio: "inherit" });
  origem = join(raiz, "target", "release");
}
mkdirSync(destino, { recursive: true });
for (const n of ["qjo-lsp", "qjo"]) {
  const de = join(origem, exe(n));
  if (!existsSync(de)) throw new Error(`não achei ${de}`);
  copyFileSync(de, join(destino, exe(n)));
  if (!win) chmodSync(join(destino, exe(n)), 0o755);
}
console.log(`servidor em ${destino}`);
