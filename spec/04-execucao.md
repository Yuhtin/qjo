# 4. Execução

O interpretador e o C++ gerado seguem este modelo e imprimem o mesmo registro, linha por linha.
O modelo é determinístico: o mesmo programa com as mesmas respostas produz sempre o mesmo registro.

## 4.1 Relógio virtual

Cada rotina tem um relógio que começa no seu horário. Um horário só com hora começa no dia
2026-11-05 (extensão do qjo). O relógio avança apenas em `ESPERAR` e em `AGUARDAR_CONFIRMACAO`.
Se uma rotina passa de duas horas desde o início, ela termina com erro.

## 4.2 Rotinas e etapas

As rotinas executam na ordem do arquivo. Cada rotina imprime `ROTINA <nome>` ao começar e
`FIM <estado>` ao terminar:

| Estado | Quando |
| --- | --- |
| `concluida` | O corpo terminou |
| `cancelada` | Um `retorna;` encerrou a rotina |
| `erro: <motivo>` | Um erro de execução encerrou a rotina |

Uma etapa imprime `ETAPA <nome>` ao começar. Ao terminar, por qualquer motivo, apaga as
indicações ainda ligadas, na ordem em que foram ligadas, imprimindo
`PARAR_INDICACAO <objeto> (fim da etapa)`. O fim da rotina faz o mesmo com `(fim da rotina)`.

## 4.3 Primitivas

| Primitiva | Efeito | Registro |
| --- | --- | --- |
| `INSTRUIR(m)` | — | `INSTRUIR <m>` |
| `INDICAR(o)` | Liga a indicação, se não estiver ligada | `INDICAR <o>` |
| `PARAR_INDICACAO(o)` | Desliga a indicação | `PARAR_INDICACAO <o>` |
| `AGUARDAR_CONFIRMACAO(d)` | Espera uma resposta; avança o relógio | `AGUARDAR_CONFIRMACAO <d>s -> <r> (<t>s)` |
| `ALERTAR(i)` | — | `ALERTAR <i>` |
| `ESPERAR(d)` | Avança o relógio em `d` | `ESPERAR <d>s` |
| `REGISTRAR(e)` | — | `REGISTRAR <e>` |
| `AGORA()` | Devolve o relógio, como data completa | — |
| `TEMPO_DE_RESPOSTA()` | Devolve o `t` da última espera, ou `0s` | — |

Objetos e mensagens aparecem pelo nome com que foram declarados. Uma primitiva que recebe `nulo`
não faz nada e não imprime nada.

**Respostas.** Cada `AGUARDAR_CONFIRMACAO(d)` consome a próxima resposta, um par
`<resultado> <segundos>`. Se não há mais respostas, se a resposta é `TEMPO_ESGOTADO` ou se os
segundos passam de `d`, o resultado é `TEMPO_ESGOTADO` com `t = d`. Com `d = 0s`, o resultado é
`TEMPO_ESGOTADO` imediatamente, sem consumir resposta. O relógio avança `t`.

## 4.4 Erros de execução

Estes erros encerram a rotina com `FIM erro: <motivo>`:

| Motivo | Situação |
| --- | --- |
| `divisão por zero` | `/` ou `%` com divisor 0 |
| `estouro de num` | Resultado fora de 32 bits |
| `duração negativa` | Duração abaixo de zero |
| `estouro de duração` | Duração acima de 2147483647 s |
| `a rotina excedeu o limite de duas horas` | Relógio passou de 2 h desde o início |
| `possível laço sem fim (limite de passos)` | Mais de 1.000.000 voltas de laço na rotina (extensão do qjo) |

## 4.5 Aritmética

A divisão inteira trunca em direção a zero e o resto tem o sinal do dividendo. `&&` e `||`
avaliam o lado direito só quando necessário. Os operandos e os argumentos são avaliados da
esquerda para a direita. Comparar datas usa todos os campos quando as duas são completas, e só a
hora do dia quando ao menos uma é só horário.
