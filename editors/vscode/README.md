# Q?JO! para VS Code e Cursor

Suporte à linguagem [Q?JO!](https://github.com/Yuhtin/qjo). Todo o entendimento da linguagem
vem do servidor `qjo-lsp`, que já vem dentro da extensão.

- **Erros enquanto você digita**, com o código (`E_LEX_LITERAL`, `E_SEM_NOME`…) e a linha certa.
- **Correção rápida** (`Cmd+.` / `Ctrl+.`): `30seg` → `30s`, `q` → `q?`, `;` que faltou, "você quis dizer camisa?".
- **Autocompletar** do que está visível no ponto, com trechos prontos (`rotina`, `etapa`, `confirmar ou cancelar`).
- **Dica** ao passar o mouse, **ir para a definição**, **referências**, **renomear** e **esboço** com rotinas e etapas.
- **Formatar** ao salvar.
- **Rodar rotina** (`Cmd+Enter` / `Ctrl+Enter` ou ▶): abre o simulador no terminal, você responde cada espera.
- **Gerar C++**: cria o `.cpp` ao lado do arquivo (`g++ -std=c++17 -O2 -o rotina rotina.cpp`).

## Outros editores

O `qjo-lsp` fala LSP pela entrada e saída padrão. Instale com
`cargo install --git https://github.com/Yuhtin/qjo qjo-lsp` e aponte seu editor para ele
(Zed, Neovim, Helix…) para arquivos `.qjo`.
