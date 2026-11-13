// Exercícios do site. Cada caso roda o programa com as respostas dadas e confere o que a rotina
// fez (como termina, eventos, alertas, esperas...). Respostas que faltarem viram TEMPO_ESGOTADO.

import type { Execucao } from "./qjo";

/** O que um caso confere, calculado a partir do registro da execução. */
export interface Medidas {
  /** Como a rotina termina: concluida, cancelada ou erro. */
  fim: string;
  eventos: string[];
  alertas: string[];
  /** Quantos AGUARDAR_CONFIRMACAO rodaram. */
  aguardas: number;
  etapas: number;
  /** Mensagens mostradas com INSTRUIR, na ordem. */
  instrucoes: string[];
  /** Objetos indicados com INDICAR, na ordem. */
  indicados: string[];
  /** Limite de cada espera, como "30s". */
  limites: string[];
}

export interface Caso {
  /** O que o caso simula, em uma frase. */
  titulo: string;
  /** Respostas do dispositivo, como na CLI: "CONFIRMADO 3, AJUDA 5". As que faltarem viram TEMPO_ESGOTADO. */
  respostas: string;
  espera: Partial<Medidas>;
}

export interface Exercicio {
  id: string;
  titulo: string;
  nivel: "fácil" | "médio" | "difícil";
  arquivo: string;
  /** Uma linha para o cartão da lista. */
  resumo: string;
  conceitos: string[];
  enunciado: string[];
  dicas: string[];
  inicial: string;
  solucao: string;
  /** Por que a solução funciona. */
  explicacao: string;
  casos: Caso[];
}

export const EXERCICIOS: Exercicio[] = [
  {
    id: "copo",
    titulo: "Um copo d'água",
    nivel: "fácil",
    arquivo: "beber_agua.qjo",
    resumo: "Uma etapa: indicar, instruir e esperar a confirmação.",
    conceitos: ["INDICAR", "INSTRUIR", "AGUARDAR_CONFIRMACAO"],
    enunciado: [
      "Na etapa “Pegar o copo”, indique o `copo`, instrua com `msg_beber` e espere a confirmação por até `30s`.",
    ],
    dicas: ["A ordem importa: o registro mostra cada primitiva na ordem em que roda.", "`AGUARDAR_CONFIRMACAO(30s)` devolve um `resultado`; guarde numa variável."],
    inicial: `# Avise a pessoa para beber água.
data tarde = data"15:00:00";
objeto copo = "Copo na mesa da sala";
mensagem msg_beber = "Beba um copo de água.";

rotina("Beber água", tarde) {
    etapa("Pegar o copo") {
        # 1. Indique o copo.
        # 2. Instrua com msg_beber.
        # 3. Espere a confirmação por até 30s.
    }
}
`,
    solucao: `# Avise a pessoa para beber água.
data tarde = data"15:00:00";
objeto copo = "Copo na mesa da sala";
mensagem msg_beber = "Beba um copo de água.";

rotina("Beber água", tarde) {
    etapa("Pegar o copo") {
        INDICAR(copo);
        INSTRUIR(msg_beber);
        resultado r = AGUARDAR_CONFIRMACAO(30s);
    }
}
`,
    explicacao: "As três primitivas rodam em sequência dentro da etapa. O resultado da espera não é usado aqui, mas a espera aparece no registro com a resposta e o tempo.",
    casos: [
      { titulo: "A pessoa confirma em 4 s", respostas: "CONFIRMADO 4", espera: { fim: "concluida", indicados: ["copo"], instrucoes: ["msg_beber"], limites: ["30s"], aguardas: 1 } },
    ],
  },
  {
    id: "ajuda",
    titulo: "Quando pedem ajuda",
    nivel: "fácil",
    arquivo: "ligar_tv.qjo",
    resumo: "Três respostas possíveis, três caminhos diferentes.",
    conceitos: ["q? qjo? jo!", "=="],
    enunciado: [
      "Depois da espera, trate as três respostas possíveis.",
      "Com `AJUDA`, alerte com intensidade `BAIXA` e instrua com `msg_dica`. Se o tempo esgotar, alerte com `MEDIA`. Se a pessoa confirmar, não faça mais nada.",
    ],
    dicas: ["Compare o resultado com `AJUDA` e com `TEMPO_ESGOTADO`.", "Confirmar não faz nada, então não precisa de `jo!`."],
    inicial: `# Ligar a TV: a pessoa pode confirmar, pedir ajuda ou não responder.
data noite = data"21:00:00";
objeto controle = "Controle remoto da TV";
mensagem msg_controle = "Pegue o controle remoto.";
mensagem msg_dica = "O controle está no braço do sofá.";

rotina("Ligar a TV", noite) {
    etapa("Pegar o controle") {
        INDICAR(controle);
        INSTRUIR(msg_controle);
        resultado r = AGUARDAR_CONFIRMACAO(40s);
        # AJUDA: ALERTAR(BAIXA) e INSTRUIR(msg_dica).
        # TEMPO_ESGOTADO: ALERTAR(MEDIA).
    }
}
`,
    solucao: `# Ligar a TV: a pessoa pode confirmar, pedir ajuda ou não responder.
data noite = data"21:00:00";
objeto controle = "Controle remoto da TV";
mensagem msg_controle = "Pegue o controle remoto.";
mensagem msg_dica = "O controle está no braço do sofá.";

rotina("Ligar a TV", noite) {
    etapa("Pegar o controle") {
        INDICAR(controle);
        INSTRUIR(msg_controle);
        resultado r = AGUARDAR_CONFIRMACAO(40s);
        q? (r == AJUDA) {
            ALERTAR(BAIXA);
            INSTRUIR(msg_dica);
        } qjo? (r == TEMPO_ESGOTADO) {
            ALERTAR(MEDIA);
        }
    }
}
`,
    explicacao: "O `q?` trata a ajuda e o `qjo?` trata o tempo esgotado. Quando a pessoa confirma, nenhum dos dois testes é verdadeiro e a etapa termina sem alerta.",
    casos: [
      { titulo: "Confirma: nenhum alerta", respostas: "CONFIRMADO 2", espera: { fim: "concluida", alertas: [], instrucoes: ["msg_controle"] } },
      { titulo: "Pede ajuda", respostas: "AJUDA 5", espera: { fim: "concluida", alertas: ["BAIXA"], instrucoes: ["msg_controle", "msg_dica"] } },
      { titulo: "Não responde", respostas: "TEMPO_ESGOTADO", espera: { fim: "concluida", alertas: ["MEDIA"], instrucoes: ["msg_controle"] } },
    ],
  },
  {
    id: "camisa-blusa",
    titulo: "Camisa e blusa",
    nivel: "médio",
    arquivo: "pegar_roupa.qjo",
    resumo: "Duas etapas; se pegar o objeto errado, a rotina é cancelada na hora.",
    conceitos: ["retorna;", "!=", "etapa"],
    enunciado: [
      "Peça a camisa e depois a blusa, cada uma na sua etapa, esperando até `1min`.",
      "Se a pessoa pegar o objeto errado (qualquer resposta que não seja `CONFIRMADO`), alerte com `ALTA`, instrua com `msg_cancela`, registre `ROTINA_INCOMPLETA` e cancele a rotina com `retorna;`.",
    ],
    dicas: ["`retorna;` dentro da rotina encerra tudo, inclusive as etapas que faltam.", "`r != CONFIRMADO` pega `AJUDA` e `TEMPO_ESGOTADO` num teste só."],
    inicial: `# Pegar a camisa e a blusa. Objeto errado cancela a rotina.
data horario = data"08:00:00";
objeto camisa = "Gaveta das camisas";
objeto blusa = "Cabide das blusas";
mensagem msg_camisa = "Pegue a camisa.";
mensagem msg_blusa = "Pegue a blusa.";
mensagem msg_cancela = "Rotina cancelada.";

rotina("Pegar a roupa", horario) {
    etapa("Pegar a camisa") {
        INDICAR(camisa);
        INSTRUIR(msg_camisa);
        resultado r = AGUARDAR_CONFIRMACAO(1min);
        PARAR_INDICACAO(camisa);
        # Se r não for CONFIRMADO, cancele a rotina.
    }
    # Falta a etapa "Pegar a blusa".
}
`,
    solucao: `# Pegar a camisa e a blusa. Objeto errado cancela a rotina.
data horario = data"08:00:00";
objeto camisa = "Gaveta das camisas";
objeto blusa = "Cabide das blusas";
mensagem msg_camisa = "Pegue a camisa.";
mensagem msg_blusa = "Pegue a blusa.";
mensagem msg_cancela = "Rotina cancelada.";

rotina("Pegar a roupa", horario) {
    etapa("Pegar a camisa") {
        INDICAR(camisa);
        INSTRUIR(msg_camisa);
        resultado r = AGUARDAR_CONFIRMACAO(1min);
        PARAR_INDICACAO(camisa);
        q? (r != CONFIRMADO) {
            ALERTAR(ALTA);
            INSTRUIR(msg_cancela);
            REGISTRAR(ROTINA_INCOMPLETA);
            retorna;
        }
    }
    etapa("Pegar a blusa") {
        INDICAR(blusa);
        INSTRUIR(msg_blusa);
        resultado r = AGUARDAR_CONFIRMACAO(1min);
        PARAR_INDICACAO(blusa);
        q? (r != CONFIRMADO) {
            ALERTAR(ALTA);
            INSTRUIR(msg_cancela);
            REGISTRAR(ROTINA_INCOMPLETA);
            retorna;
        }
    }
}
`,
    explicacao: "Cada etapa para a indicação antes de testar a resposta, para não deixar o objeto aceso ao cancelar. `retorna;` encerra a rotina inteira, então a etapa da blusa nem começa se a camisa falhar.",
    casos: [
      { titulo: "Pega as duas peças", respostas: "CONFIRMADO 3, CONFIRMADO 5", espera: { fim: "concluida", eventos: [], alertas: [], aguardas: 2, etapas: 2 } },
      { titulo: "Erra a camisa: pede ajuda", respostas: "AJUDA 4", espera: { fim: "cancelada", eventos: ["ROTINA_INCOMPLETA"], alertas: ["ALTA"], aguardas: 1, etapas: 1, instrucoes: ["msg_camisa", "msg_cancela"] } },
      { titulo: "Pega a camisa, não pega a blusa", respostas: "CONFIRMADO 3, TEMPO_ESGOTADO", espera: { fim: "cancelada", eventos: ["ROTINA_INCOMPLETA"], alertas: ["ALTA"], aguardas: 2, etapas: 2 } },
    ],
  },
  {
    id: "tres-tentativas",
    titulo: "Três tentativas",
    nivel: "médio",
    arquivo: "sair_de_casa.qjo",
    resumo: "Até três pedidos com um laço; para de pedir depois que a pessoa confirma.",
    conceitos: ["para", "logico", "!"],
    enunciado: [
      "Instrua com `msg_chave` e espere `20s`, até três vezes. Pare assim que a pessoa confirmar.",
      "Se ela não confirmar em nenhuma das três, registre `ETAPA_PULADA`.",
    ],
    dicas: ["Guarde o estado num `logico pegou = falso;`", "A condição do `para` pode ser `t <= 3 && !pegou`."],
    inicial: `# Sair de casa com a chave.
data saida = data"07:40:00";
objeto chave = "Chaveiro ao lado da porta";
mensagem msg_chave = "Pegue a chave de casa.";

rotina("Sair de casa", saida) {
    etapa("Pegar a chave") {
        INDICAR(chave);
        INSTRUIR(msg_chave);
        resultado r = AGUARDAR_CONFIRMACAO(20s);
    }
}
`,
    solucao: `# Sair de casa com a chave.
data saida = data"07:40:00";
objeto chave = "Chaveiro ao lado da porta";
mensagem msg_chave = "Pegue a chave de casa.";

rotina("Sair de casa", saida) {
    etapa("Pegar a chave") {
        INDICAR(chave);
        logico pegou = falso;
        para (num t = 1; t <= 3 && !pegou; t = t + 1) {
            INSTRUIR(msg_chave);
            resultado r = AGUARDAR_CONFIRMACAO(20s);
            pegou = r == CONFIRMADO;
        }
        q? (!pegou) {
            REGISTRAR(ETAPA_PULADA);
        }
    }
}
`,
    explicacao: "O laço para por qualquer um dos dois motivos: acabaram as três voltas ou a pessoa confirmou. Depois dele, `!pegou` só é verdadeiro se nenhuma tentativa deu certo.",
    casos: [
      { titulo: "Confirma na primeira", respostas: "CONFIRMADO 2", espera: { fim: "concluida", aguardas: 1, eventos: [] } },
      { titulo: "Confirma na terceira", respostas: "AJUDA 3, TEMPO_ESGOTADO, CONFIRMADO 4", espera: { fim: "concluida", aguardas: 3, eventos: [] } },
      { titulo: "Nunca responde", respostas: "", espera: { fim: "concluida", aguardas: 3, limites: ["20s", "20s", "20s"], eventos: ["ETAPA_PULADA"] } },
    ],
  },
  {
    id: "alerta-cresce",
    titulo: "Alerta que cresce",
    nivel: "difícil",
    arquivo: "remedio_noite.qjo",
    resumo: "Uma função escolhe a intensidade do alerta a cada tentativa.",
    conceitos: ["funcao", "retorna", "intensidade"],
    enunciado: [
      "Faça `alerta_para` devolver `BAIXA` na 1ª tentativa, `MEDIA` na 2ª e `ALTA` a partir da 3ª.",
      "Depois do laço, se a pessoa não tomou o remédio, registre `ROTINA_INCOMPLETA`.",
    ],
    dicas: ["Uma função pode ter vários `retorna`; o primeiro que rodar encerra a função.", "Depois de testar 1 e 2, o resto é `ALTA`."],
    inicial: `# O alerta fica mais forte a cada tentativa sem resposta.
data noite = data"20:00:00";
objeto caixa = "Caixa de remédios da noite";
mensagem msg_remedio = "Tome o remédio da noite.";

funcao intensidade alerta_para(num tentativa) {
    retorna BAIXA;
}

rotina("Remédio da noite", noite) {
    etapa("Tomar o remédio") {
        INDICAR(caixa);
        logico tomou = falso;
        para (num t = 1; t <= 3 && !tomou; t = t + 1) {
            INSTRUIR(msg_remedio);
            resultado r = AGUARDAR_CONFIRMACAO(1min);
            q? (r == CONFIRMADO) {
                tomou = verdadeiro;
            } jo! {
                ALERTAR(alerta_para(t));
            }
        }
    }
}
`,
    solucao: `# O alerta fica mais forte a cada tentativa sem resposta.
data noite = data"20:00:00";
objeto caixa = "Caixa de remédios da noite";
mensagem msg_remedio = "Tome o remédio da noite.";

funcao intensidade alerta_para(num tentativa) {
    q? (tentativa == 1) {
        retorna BAIXA;
    } qjo? (tentativa == 2) {
        retorna MEDIA;
    }
    retorna ALTA;
}

rotina("Remédio da noite", noite) {
    etapa("Tomar o remédio") {
        INDICAR(caixa);
        logico tomou = falso;
        para (num t = 1; t <= 3 && !tomou; t = t + 1) {
            INSTRUIR(msg_remedio);
            resultado r = AGUARDAR_CONFIRMACAO(1min);
            q? (r == CONFIRMADO) {
                tomou = verdadeiro;
            } jo! {
                ALERTAR(alerta_para(t));
            }
        }
        q? (!tomou) {
            REGISTRAR(ROTINA_INCOMPLETA);
        }
    }
}
`,
    explicacao: "`alerta_para` devolve um valor em cada caminho, e o último `retorna ALTA;` cobre da terceira tentativa em diante. O laço já existia; faltava registrar a falha depois dele.",
    casos: [
      { titulo: "Toma na segunda tentativa", respostas: "TEMPO_ESGOTADO, CONFIRMADO 8", espera: { aguardas: 2, alertas: ["BAIXA"], eventos: [] } },
      { titulo: "Pede ajuda três vezes", respostas: "AJUDA 5, AJUDA 9, AJUDA 2", espera: { alertas: ["BAIXA", "MEDIA", "ALTA"], eventos: ["ROTINA_INCOMPLETA"] } },
      { titulo: "Nunca responde", respostas: "", espera: { fim: "concluida", aguardas: 3, alertas: ["BAIXA", "MEDIA", "ALTA"], eventos: ["ROTINA_INCOMPLETA"] } },
    ],
  },
];

/** Uma linha da tabela "Confere / Esperado / Obtido". */
export interface Conferencia {
  rotulo: string;
  esperado: string;
  obtido: string;
  ok: boolean;
}

export interface ResultadoCaso {
  caso: Caso;
  ok: boolean;
  conferencias: Conferencia[];
  /** Erro de execução ou rotina que nunca para de esperar. */
  erro: string | null;
  registro: string[];
}

const LIMITE_RESPOSTAS = 50;

const ROTULO: Record<keyof Medidas, string> = {
  fim: "Como termina",
  eventos: "Eventos registrados",
  alertas: "Alertas",
  aguardas: "Pedidos de confirmação",
  etapas: "Etapas iniciadas",
  instrucoes: "Mensagens mostradas",
  indicados: "Objetos indicados",
  limites: "Limite de cada espera",
};

const FIM: Record<string, string> = {
  concluida: "chega ao fim",
  cancelada: "encerra com retorna;",
  erro: "erro de execução",
};

function fmtValor(k: keyof Medidas, v: unknown): string {
  if (k === "fim") return FIM[String(v)] ?? String(v);
  if (Array.isArray(v)) return v.length ? v.join(", ") : "(nenhum)";
  return String(v);
}

export function medir(registro: string[]): Medidas {
  const m: Medidas = { fim: "", eventos: [], alertas: [], aguardas: 0, etapas: 0, instrucoes: [], indicados: [], limites: [] };
  for (const l of registro) {
    const [cmd, ...resto] = l.split(" ");
    const arg = resto.join(" ");
    if (cmd === "REGISTRAR") m.eventos.push(arg);
    else if (cmd === "ALERTAR") m.alertas.push(arg);
    else if (cmd === "ETAPA") m.etapas++;
    else if (cmd === "INSTRUIR") m.instrucoes.push(arg);
    else if (cmd === "INDICAR") m.indicados.push(arg);
    else if (cmd === "AGUARDAR_CONFIRMACAO") {
      m.aguardas++;
      m.limites.push(resto[0]);
    } else if (cmd === "FIM") m.fim = arg.startsWith("erro") ? "erro" : arg;
  }
  return m;
}

/** Roda um caso sem pausar: cada espera sem resposta vira TEMPO_ESGOTADO. */
export function rodarCaso(
  executar: (src: string, respostas: string) => Execucao,
  src: string,
  caso: Caso,
): ResultadoCaso {
  let respostas = caso.respostas;
  let r = executar(src, respostas);
  for (let i = 0; r.ok && r.pausada !== null && i < LIMITE_RESPOSTAS; i++) {
    respostas = respostas.trim() ? `${respostas}, TEMPO_ESGOTADO` : "TEMPO_ESGOTADO";
    r = executar(src, respostas);
  }
  if (!r.ok) {
    return { caso, ok: false, conferencias: [], erro: r.erro ?? "o programa tem erros e não roda", registro: [] };
  }
  const m = medir(r.registro);
  const conferencias = (Object.keys(caso.espera) as (keyof Medidas)[]).map((k) => ({
    rotulo: ROTULO[k],
    esperado: fmtValor(k, caso.espera[k]),
    obtido: fmtValor(k, m[k]),
    ok: JSON.stringify(caso.espera[k]) === JSON.stringify(m[k]),
  }));
  const erroFim = r.registro.find((l) => l.startsWith("FIM erro"));
  const erro = r.pausada !== null
    ? `a rotina continuou esperando depois de ${LIMITE_RESPOSTAS} respostas`
    : erroFim ? erroFim.replace(/^FIM erro:\s*/, "") : null;
  return { caso, ok: erro === null && conferencias.every((x) => x.ok), conferencias, erro, registro: r.registro };
}
