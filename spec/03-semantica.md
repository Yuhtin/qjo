# 3. Semântica

## 3.1 Tipos

| Tipo | Valores |
| --- | --- |
| `num` | Inteiro de 32 bits com sinal |
| `logico` | `verdadeiro`, `falso` |
| `texto` | Cadeia; admite `==` e `!=`, sem concatenação |
| `duracao` | Segundos inteiros, de 0 a 2147483647 |
| `data` | Data completa ou só horário |
| `objeto`, `mensagem` | Recursos declarados no topo, ou `nulo` |
| `resultado` | `CONFIRMADO`, `AJUDA`, `TEMPO_ESGOTADO`, ou `nulo` |
| `intensidade` | `BAIXA`, `MEDIA`, `ALTA` |
| `evento` | `ETAPA_PULADA`, `AJUDA_REPETIDA`, `CONFIRMACAO_RAPIDA`, `ROTINA_INCOMPLETA` |
| enum declarado | Os valores listados na declaração |

Não há conversão implícita. `nulo` só vale para `objeto`, `mensagem` e `resultado`.
`enum` sozinho não é um tipo de variável: ele só declara conjuntos no topo.

## 3.2 Topo do arquivo

- `objeto nome = "descrição";` e `mensagem nome = "descrição";` declaram recursos constantes.
  A descrição é um `TEXTO` não vazio (`E_SEM_RECURSO`).
- `data nome = data"…";` declara um horário constante. O valor é um literal `DATA`.
- Qualquer outra declaração no topo gera `E_SEM_GLOBAL`.
- `enum Nome { A, B }` declara um conjunto. Valores de enum têm nomes únicos entre todos os
  conjuntos, inclusive os predefinidos (`E_SEM_REDECL`).
- O arquivo precisa de ao menos uma `rotina` (`E_SEM_ROTINA`).
- Funções podem ser usadas antes de sua declaração. Recursos, horários e enums também.

## 3.3 Nomes e escopo

- Todo nome usado precisa existir (`E_SEM_NOME`, com sugestão do nome mais parecido).
- Uma variável local existe da declaração até o fim do bloco. A variável do `para` existe só no
  laço. Parâmetros existem no corpo da função.
- Nenhum nome pode repetir um nome visível: local, parâmetro, recurso, horário, função, enum,
  valor de enum ou primitiva (`E_SEM_REDECL`).
- Recursos e horários são constantes; valores de enum, funções e primitivas não são variáveis.
  Atribuir a eles gera `E_SEM_ATRIB`.

## 3.4 Comandos

- Um comando de expressão precisa ser uma chamada (`E_SEM_CMD`).
- `etapa` só aparece dentro de uma rotina, fora de outra etapa (`E_SEM_ETAPA`). Nomes de etapa e
  de rotina não podem ser vazios nem só espaços (`E_SEM_NOME_VAZIO`).
- O horário da rotina tem tipo `data` (`E_SEM_TIPO`).
- Condições de `q?`, `qjo?`, `enquanto`, `faca` e `para` têm tipo `logico` (`E_SEM_TIPO`).
- No `para`, a variável é um `num` novo e a atualização atribui a ela mesma (`E_SEM_PARA`).
- Variáveis locais de `objeto` e `mensagem` recebem um recurso existente ou `nulo`, nunca um
  texto (`E_SEM_RECURSO`).
- Numa função com tipo, todo caminho termina em `retorna valor;` do tipo certo. Numa função sem
  tipo e numa rotina, só `retorna;` (`E_SEM_RETORNO`).
- Recursão, direta ou indireta, gera `E_SEM_RECURSAO`.

## 3.5 Expressões

| Operação | Tipos | Resultado |
| --- | --- | --- |
| `+ - * / %` | `num`, `num` | `num` |
| `+ -` | `duracao`, `duracao` | `duracao` |
| `*` | `duracao`, `num` ou `num`, `duracao` | `duracao` |
| `/` | `duracao`, `num` | `duracao` |
| `< > <= >=` | dois `num`, duas `duracao` ou duas `data` | `logico` |
| `== !=` | dois valores do mesmo tipo, ou `nulo` com `objeto`/`mensagem`/`resultado` | `logico` |
| `&& \|\|` | `logico`, `logico` | `logico` |
| `!` | `logico` | `logico` |
| `-` unário | `num` | `num` |

Outras combinações geram `E_SEM_TIPO`, apontando o operador. Uma chamada confere a quantidade e o tipo de cada
argumento. Uma função sem tipo não pode ser usada como valor.

**Constantes.** Quando os dois operandos são literais, ou o divisor é o literal `0`, a divisão ou
o resto por zero e o estouro são erros de compilação (`E_SEM_CONSTANTE`, extensão do qjo).
Uma duração literal acima de 2147483647 segundos gera `E_SEM_DURACAO`. Uma data com calendário
ou horário impossível gera `E_SEM_DATA`.

## 3.6 Erros

| Código | Situação |
| --- | --- |
| `E_SEM_NOME` | Nome não declarado |
| `E_SEM_REDECL` | Nome repetido ou que oculta outro |
| `E_SEM_TIPO` | Tipo incompatível |
| `E_SEM_ATRIB` | Atribuição a algo que não é variável |
| `E_SEM_CMD` | Expressão sozinha que não é chamada |
| `E_SEM_GLOBAL` | Declaração no topo que não é recurso nem horário |
| `E_SEM_RECURSO` | Descrição vazia, ou recurso criado fora do topo |
| `E_SEM_ETAPA` | Etapa fora de rotina ou dentro de outra etapa |
| `E_SEM_NOME_VAZIO` | Rotina ou etapa com nome vazio |
| `E_SEM_PARA` | Cabeçalho do `para` fora da regra |
| `E_SEM_RETORNO` | `retorna` incompatível ou caminho sem retorno |
| `E_SEM_RECURSAO` | Recursão |
| `E_SEM_ROTINA` | Arquivo sem rotina |
| `E_SEM_DURACAO` | Duração literal grande demais |
| `E_SEM_DATA` | Data impossível |
| `E_SEM_CONSTANTE` | Divisão por zero ou estouro conhecidos na compilação |
