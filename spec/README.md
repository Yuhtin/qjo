# Especificação da Q?JO!

Esta é a referência da linguagem implementada neste repositório. Quando o código e este texto
discordam, o texto manda e o código está errado — ou o texto é corrigido no mesmo commit.

| Parte | Conteúdo |
| --- | --- |
| [1. Léxico](01-lexico.md) | Tokens, posições, erros `E_LEX_*` |
| [2. Sintaxe](02-sintaxe.md) | Gramática, erros `E_SINT`, recuperação |
| [3. Semântica](03-semantica.md) | Tipos, escopo, regras, erros `E_SEM_*` |
| [4. Execução](04-execucao.md) | Relógio virtual, primitivas, limites, formato do registro |
| [5. C++](05-cpp.md) | O programa C++ gerado e seu protocolo de entrada e saída |

## Compatibilidade

O léxico segue a definição original da linguagem: os mesmos tokens, os mesmos códigos de erro,
linha e coluna contadas do mesmo jeito. Tudo o que a definição original deixa em aberto e este
repositório decide está marcado como **extensão do qjo**.

## Fases

Um programa passa por léxico, sintaxe e semântica, nessa ordem. Se uma fase encontra erros, as
seguintes não rodam: os erros de uma fase pressupõem que a anterior deu certo, e rodar adiante só
produziria erros em cascata. Dentro de cada fase, todos os erros encontrados são relatados.

## Diagnósticos

Todo diagnóstico tem arquivo, linha, coluna, código e mensagem. Linha e coluna começam em 1 e
indicam o início do trecho com problema. Quando há uma correção evidente, o diagnóstico traz uma
**sugestão**: um título curto ("Escrever 30s") e as edições que a aplicam. Mensagens e sugestões
podem melhorar entre versões; códigos, linhas e colunas são estáveis e são o que os testes conferem.
