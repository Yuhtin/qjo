# qjo

Monorepo da linguagem Q?JO!. Leia o `README.md` para o mapa das pastas.

## Regras

- **A spec manda.** Toda mudança de comportamento da linguagem começa em `spec/`, no mesmo
  commit que muda o código. Decisões que vão além da definição original são marcadas como
  "extensão do qjo".
- **Conformidade antes de tudo.** `cargo test --all` precisa passar. Um caso novo em
  `tests/conformance` para cada regra nova ou bug corrigido. Para regenerar os esperados use
  `QJO_ATUALIZAR=1 cargo test -p qjo-core --test conformidade` e revise o diff linha a linha:
  o esperado é a especificação executável, não uma foto do que o código faz.
- O C++ gerado e o interpretador imprimem exatamente o mesmo registro (`tests/cpp.rs` confere).
- `qjo-core` não tem dependências. O LSP pode ter; o WASM não usa wasm-bindgen.
- Desempenho: siga `.claude/skills/optimize-app` — meça antes e depois com `bench/`, reverta o
  que não ganhou e registre os números no commit.
- Código, mensagens e documentação em português.
