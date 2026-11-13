# 1. Léxico

## 1.1 Entrada e posições

- O fonte é UTF-8. Espaço, tabulação, LF e CRLF separam lexemas e não geram tokens.
- CR sozinho não é separador: gera `E_LEX_SIMBOLO`.
- Linha e coluna começam em 1 e marcam o início do lexema. A coluna conta pontos de código
  Unicode; uma tabulação ocupa uma coluna.
- Maiúsculas e minúsculas são distintas: `num` é `TIPO`, `Num` é `ID`.
- Comentários geram `COMENT`; a sintaxe os descarta.

## 1.2 Tokens

| Lexema | Token |
| --- | --- |
| `q?` | `SE` |
| `qjo?` | `SENAOSE` |
| `jo!` | `SENAO` |
| `para`, `enquanto`, `faca`, `retorna`, `rotina`, `etapa`, `funcao` | `PARA`, `ENQUANTO`, `FACA`, `RETORNA`, `ROTINA`, `ETAPA`, `FUNCAO` |
| `verdadeiro`, `falso` | `LOGICO` |
| `nulo` | `NULO` |
| `num`, `texto`, `logico`, `enum`, `data`, `duracao`, `objeto`, `mensagem`, `resultado`, `evento`, `intensidade` | `TIPO` |
| `+` `-` | `OPSOMA` |
| `*` `/` `%` | `OPMULT` |
| `<` `>` | `OPREL` |
| `<=` `>=` | `OPRELIGUAL` |
| `==` | `IGUAL` |
| `!=` | `DIFERENTE` |
| `&&` | `E` |
| `\|\|` | `OU` |
| `!` | `NAO` |
| `=` | `ATRIBUICAO` |
| `(` `)` `{` `}` | `ABREPAR` `FECHAPAR` `ABRECHAVE` `FECHACHAVE` |
| `,` | `SEP` |
| `;` | `PONTOVIRGULA` |
| `[A-Za-z][A-Za-z0-9_]*` que não é palavra reservada | `ID` |
| `[0-9]+` com valor até 2147483647 | `INTEIRO` |
| aspas duplas na mesma linha | `TEXTO` |
| um a três grupos `Nh`, `Nmin`, `Ns`, nessa ordem | `DURACAO` |
| `data"AAAA-MM-DD HH:MM:SS"` ou `data"HH:MM:SS"` | `DATA` |
| `#` até o fim da linha | `COMENT` |

Nomes das primitivas, de recursos e de valores de enumeração são `ID`; a semântica confere se
existem. Não há token de fim de arquivo na saída do léxico.

## 1.3 Ordem de reconhecimento

1. Separadores.
2. Texto (`"`) e comentário (`#`).
3. `qjo?`, `q?`, `jo!`.
4. `data"` antes de `data` e de identificador.
5. Candidato numérico: a maior sequência de `[A-Za-z0-9_]` que começa com dígito.
6. Identificador inteiro, depois consulta às palavras reservadas.
7. Operadores de dois caracteres (`<=`, `>=`, `==`, `!=`, `&&`, `||`) antes dos de um.

## 1.4 Regras

**Palavras reservadas.** Uma palavra reservada precisa coincidir com o identificador inteiro:
`rotina_ativa` é `ID`. `q`, `qjo` e `jo` sozinhos geram `E_LEX_RESERVADA` e nenhum token.
`meuqjo?` gera `ID("meuqjo")` e `E_LEX_SIMBOLO` no `?`.

**Inteiros.** O sinal não faz parte do literal: `-10` é `OPSOMA` e `INTEIRO`. O teto é
conferido pelos dígitos (zeros à esquerda não contam), sem converter o número. Por isso
`-2147483648` não pode ser escrito como literal: `2147483648` gera `E_LEX_LITERAL`.

**Durações.** O candidato numérico inteiro precisa casar com
`(N h)? (N min)? (N s)?`, com ao menos um grupo. Se não casar (`30seg`, `1h2h`, `30s1min`,
`2minx`), o candidato inteiro gera `E_LEX_LITERAL`, sem token parcial. `2 min` são dois tokens
(`INTEIRO`, `ID`) e `1h 30min` são duas durações. O total máximo é verificado na semântica.

**Texto.** O lexema inclui as aspas. Escapes válidos: `\"`, `\\`, `\n`. Outro escape gera
`E_LEX_ESCAPE` na barra; o texto continua sendo lido até a aspa final, mas não gera token.
Uma quebra de linha ou o fim do arquivo antes da aspa final gera `E_LEX_TEXTO` na aspa inicial,
e a leitura continua na linha seguinte. `#` dentro de texto não é comentário.

**Datas.** `data` seguido imediatamente de `"` é candidato a `DATA`. Ele precisa ter exatamente
`AAAA-MM-DD HH:MM:SS` ou `HH:MM:SS` em dígitos ASCII. Caso contrário, gera `E_LEX_DATA` no `d`
e consome até a aspa final ou até o fim da linha. A validade do calendário é semântica.
`data "07:30:00"`, com espaço, é `TIPO` seguido de `TEXTO`.

**Símbolos.** `&`, `|`, `?`, `@`, `[`, `]`, `.` e qualquer outro caractere sem token geram
`E_LEX_SIMBOLO`, e a leitura continua no caractere seguinte. `1.5` gera `INTEIRO`,
`E_LEX_SIMBOLO` e `INTEIRO`. `//` são dois `OPMULT`.

## 1.5 Erros

| Código | Situação | Onde aponta |
| --- | --- | --- |
| `E_LEX_SIMBOLO` | Caractere sem token | O caractere |
| `E_LEX_LITERAL` | Candidato numérico malformado ou inteiro acima de 2147483647 | Início do candidato |
| `E_LEX_RESERVADA` | `q`, `qjo` ou `jo` sozinhos | A palavra |
| `E_LEX_TEXTO` | Texto sem aspa final na linha | A aspa inicial |
| `E_LEX_ESCAPE` | Escape fora de `\"`, `\\`, `\n` | A barra |
| `E_LEX_DATA` | Candidato `data"` malformado ou sem fechamento | O `d` de `data` |

A recuperação nunca insere caracteres. Tokens válidos antes e depois de um erro são mantidos.

**Sugestões (extensão do qjo).** O léxico sugere: `q?`/`qjo?`/`jo!` para a palavra sem
pontuação; a duração válida mais próxima para `30seg` (`30s`), `2mins` (`2min`) e afins;
`!=`, `<=`, `>=`, `&&`, `||` e `"` para `≠`, `≤`, `≥`, `&`, `|` e aspas tipográficas; a letra
sem acento para uma letra acentuada fora de texto; fechar a aspa no fim da linha; trocar `\t`
por espaço.
