# Q?JO!

**Q?JO!** (lê-se "queijo") é uma linguagem pequena para descrever rotinas assistivas: uma
sequência de etapas em que um dispositivo indica objetos, dá instruções e espera a pessoa
confirmar. Ela nasceu como linguagem de um projeto de Ciência da Computação no Inteli. Este
repositório é um projeto pessoal: pensei em fazer várias coisas com ela, e aqui estão
todas, sobre um mesmo compilador escrito do zero em Rust.

```qjo
data horario = data"08:00:00";
objeto camisa = "Gaveta das camisas";
mensagem msg_camisa = "Pegue a camisa.";

rotina("Pegar a camisa", horario) {
    etapa("Pegar a camisa") {
        INDICAR(camisa);
        INSTRUIR(msg_camisa);
        resultado r = AGUARDAR_CONFIRMACAO(1min);
        q? (r != CONFIRMADO) {
            REGISTRAR(ROTINA_INCOMPLETA);
            retorna;
        }
    }
}
```

`q?` é *se*, `qjo?` é *senão se* e `jo!` é *senão*. A linguagem inteira está em [`spec/`](spec/).

## O que tem aqui

| Pasta | O que é |
| --- | --- |
| [`spec/`](spec/) | A especificação: léxico, sintaxe, semântica, execução e C++. Ela manda no código. |
| [`crates/qjo-core`](crates/qjo-core) | Léxico, sintaxe (com recuperação de erros), semântica, interpretador, gerador de C++17 e formatador. Sem dependências. |
| [`crates/qjo-cli`](crates/qjo-cli) | O comando `qjo`: `check`, `run`, `build`, `tokens`, `fmt`. |
| [`crates/qjo-lsp`](crates/qjo-lsp) | Servidor de linguagem: serve VS Code, Cursor, Zed, Neovim, Helix e qualquer editor com LSP. |
| [`crates/qjo-wasm`](crates/qjo-wasm) | O núcleo compilado para WebAssembly (206 KB, sem wasm-bindgen). |
| [`web/`](web/) | Playground no navegador: editor com erros e correções, simulador e exercícios. |
| [`editors/vscode`](editors/vscode) | Extensão para VS Code e Cursor, que leva o `qjo-lsp` e o `qjo` dentro. |
| [`tests/conformance`](tests/conformance) | Programas `.qjo` com a saída esperada. Todas as implementações passam por eles. |
| [`examples/`](examples/) | Rotinas de exemplo. |

## Usar

```sh
cargo install --git https://github.com/Yuhtin/qjo qjo-cli qjo-lsp

qjo check rotina.qjo                      # erros com linha, coluna e sugestão
qjo run rotina.qjo                        # simulador: você responde cada espera no terminal
qjo run rotina.qjo -r "CONFIRMADO 3, AJUDA 5"
qjo build rotina.qjo --exe                # gera rotina.cpp e compila com o C++ do sistema
qjo fmt rotina.qjo
```

```text
erro[E_LEX_LITERAL]: literal malformado: 30seg; uma duração usa h, min e s, nessa ordem e sem repetir
 --> rotina.qjo:9:44
  |
9 |         resultado r = AGUARDAR_CONFIRMACAO(30seg);
  |                                            ^^^^^
  = sugestão: Escrever 30s

1 erro de léxico em rotina.qjo
```

A extensão do VS Code e do Cursor sai em cada [release](https://github.com/Yuhtin/qjo/releases),
um `.vsix` por plataforma (Extensions → Install from VSIX). Para Zed, Neovim ou Helix, aponte o
editor para o `qjo-lsp` em arquivos `.qjo`.

## Como os testes seguram o comportamento

- `tests/conformance` tem 56 casos, cada um conferindo uma regra da spec: posições do léxico,
  recuperação de erros de sintaxe, cada erro semântico e cada caso de execução.
- O C++ gerado é compilado com `-Wall -Wextra -Werror` e precisa imprimir exatamente o
  mesmo que o interpretador, com o mesmo código de saída.
- O servidor de linguagem é testado conversando com o binário por JSON-RPC, e a extensão
  dentro de um VS Code de verdade.

```sh
cargo test --all                          # núcleo, conformidade, C++ e LSP
QJO_ATUALIZAR=1 cargo test -p qjo-core --test conformidade   # reescreve os esperados
```

## Desempenho

Comparado com a primeira implementação, em JavaScript (Node 22), nas mesmas entradas; mediana
de 30 rodadas num Mac com Apple Silicon. A análise inclui léxico, sintaxe e semântica.

| Medida | JavaScript | Rust | |
| --- | --- | --- | --- |
| Análise, 900 linhas (200 rodadas) | 0,79 ms | 0,27 ms | 2,9× |
| Análise, 9 mil linhas | 9,6 ms | 2,6 ms | 3,7× |
| Análise, 90 mil linhas | 104 ms | 32,5 ms | 3,2× |
| Execução, 150 mil voltas de laço com chamada | 517 ms | 21,2 ms | 24× |

Os programas e o medidor estão em [`bench/`](bench/).

## Próximos passos

- Gramática tree-sitter para cores nativas no Zed, Neovim e Helix.
- Publicar a extensão no Marketplace do VS Code e no Open VSX (o Cursor busca de lá).
- Pedir a entrada da linguagem no Linguist, para o GitHub colorir arquivos `.qjo`.
- Compilar para bytecode, se algum dia as rotinas ficarem grandes o bastante para importar.

## Licença

[MIT](LICENSE).
