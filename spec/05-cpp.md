# 5. C++

`qjo build rotina.qjo` gera um único arquivo C++17, sem dependências além da biblioteca padrão.
Ele compila com `g++ -std=c++17` ou `clang++ -std=c++17`.

## 5.1 Protocolo

- **Entrada:** uma resposta por linha, no formato `<RESULTADO> <segundos>`, por exemplo
  `CONFIRMADO 3`. Os segundos são opcionais (padrão 0). Linhas vazias e linhas que começam
  com `#` são ignoradas. O fim da entrada vale `TEMPO_ESGOTADO` para todas as esperas seguintes.
- **Saída:** exatamente o registro descrito em [Execução](04-execucao.md), uma linha por
  evento, terminada em `\n`.
- **Código de saída:** `0` quando todas as rotinas terminam como `concluida` ou `cancelada`,
  `1` quando alguma termina com `erro`.

O mesmo programa com a mesma entrada produz a mesma saída no interpretador (`qjo run`) e no
binário C++. Os testes de conformidade conferem isso.

## 5.2 Tradução

| Q?JO! | C++ |
| --- | --- |
| `num` | `int32_t`, com operações conferidas contra estouro |
| `duracao` | `int64_t` conferido entre 0 e 2147483647 |
| `data` | estrutura com indicador de data completa e segundos |
| `texto` | `std::string` |
| `objeto`, `mensagem` | ponteiro para o recurso; `nulo` é `nullptr` |
| enums | inteiro; `nulo` de `resultado` é `-1` |
| `etapa`, `rotina` | objeto cujo destrutor apaga as indicações |
| `retorna;` na rotina | exceção capturada no fim da rotina |
| erro de execução | exceção capturada no fim da rotina |

Quando dois operandos ou dois argumentos contêm chamadas, o código gerado os avalia em
temporários, na ordem, porque o C++ não garante a ordem de avaliação de argumentos.
