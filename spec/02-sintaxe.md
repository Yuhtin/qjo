# 2. Sintaxe

## 2.1 Gramática

Notação: `{x}` repete zero ou mais vezes, `[x]` é opcional, maiúsculas são tokens.

```text
programa    = { item } ;
item        = recurso | global | enum | funcao | rotina ;

recurso     = TIPO ID "=" expr ";" ;                (* objeto, mensagem, data e as demais *)
enum        = "enum" ID "{" ID { "," ID } "}" [";"] ;
funcao      = "funcao" [tipo] ID "(" [param { "," param }] ")" bloco ;
param       = tipo ID ;
rotina      = "rotina" "(" TEXTO "," expr ")" bloco ;

tipo        = TIPO | ID ;                            (* ID é um enum declarado *)
bloco       = "{" { comando } "}" ;

comando     = declaracao | atribuicao | chamada ";" | se | enquanto | faca | para
            | retorna | etapa ;
declaracao  = tipo ID "=" expr ";" ;
atribuicao  = ID "=" expr ";" ;
se          = "q?" "(" expr ")" bloco { "qjo?" "(" expr ")" bloco } [ "jo!" bloco ] ;
enquanto    = "enquanto" "(" expr ")" bloco ;
faca        = "faca" bloco "enquanto" "(" expr ")" ";" ;
para        = "para" "(" tipo ID "=" expr ";" expr ";" ID "=" expr ")" bloco ;
retorna     = "retorna" [expr] ";" ;
etapa       = "etapa" "(" TEXTO ")" bloco ;

expr        = ou ;
ou          = e { "||" e } ;
e           = igualdade { "&&" igualdade } ;
igualdade   = relacional { ("==" | "!=") relacional } ;
relacional  = soma { ("<" | ">" | "<=" | ">=") soma } ;
soma        = produto { ("+" | "-") produto } ;
produto     = unario { ("*" | "/" | "%") unario } ;
unario      = ("!" | "-") unario | primario ;
primario    = INTEIRO | DURACAO | DATA | TEXTO | LOGICO | NULO
            | ID [ "(" [expr { "," expr }] ")" ] | "(" expr ")" ;
```

Todos os operadores binários associam à esquerda. Um comando que começa com uma expressão
qualquer (`x + 1;`) é aceito pela sintaxe e recusado pela semântica (`E_SEM_CMD`), para que a
mensagem explique o que está errado.

**Extensões do qjo.** A definição original não fixa a forma das funções nem dos enums; as
formas acima são as deste repositório. Uma função sem tipo antes do nome não devolve valor.
Uma declaração no topo com tipo diferente de `objeto`, `mensagem` e `data` é aceita pela sintaxe
e recusada pela semântica (`E_SEM_GLOBAL`).

## 2.2 Erros

Todo erro de sintaxe usa o código `E_SINT` e aponta o token inesperado, com uma mensagem no
formato "esperava X, mas veio Y". Quando o token que falta é `;` ou `)`, o erro aponta o fim do
token anterior e sugere inseri-lo ali — é onde a pessoa esqueceu.

**Recuperação.** Depois de um erro, o analisador descarta tokens até um ponto seguro e
continua, para relatar outros erros independentes na mesma passada:

- dentro de um bloco: até depois do próximo `;`, ou até um `}` ou o início de um comando
  (`q?`, `enquanto`, `faca`, `para`, `retorna`, `etapa`);
- no topo: até o próximo `rotina`, `funcao`, `enum` ou tipo no início de uma linha.

Erros que começam no mesmo token do erro anterior não são repetidos.
