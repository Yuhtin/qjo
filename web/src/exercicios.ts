// Exercícios do playground. Cada caso roda o programa com as respostas dadas; se ele pausar
// pedindo mais respostas, cada espera extra vira TEMPO_ESGOTADO.

import type { Execucao } from "./qjo";

export interface Caso {
  /** O que o caso simula, em uma frase. */
  titulo: string;
  respostas: string;
  /** Linhas que precisam aparecer no registro (linha inteira). */
  contem?: string[];
  /** Linhas que não podem aparecer. */
  naoContem?: string[];
  /** Quantas vezes uma linha aparece. */
  vezes?: Record<string, number>;
  /** Estado final de cada rotina, na ordem do arquivo. */
  estados?: string[];
}

export interface Exercicio {
  id: string;
  titulo: string;
  tema: string;
  enunciado: string[];
  inicial: string;
  casos: Caso[];
}

export const EXERCICIOS: Exercicio[] = [
  {
    id: "copo",
    titulo: "Um copo d'água",
    tema: "Primitivas",
    enunciado: [
      "Na etapa “Pegar o copo”, indique o `copo`, instrua com `msg_beber` e espere a confirmação por até `30s`.",
    ],
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
    casos: [
      {
        titulo: "A pessoa confirma em 4 s",
        respostas: "CONFIRMADO 4",
        contem: [
          "ETAPA Pegar o copo",
          "INDICAR copo",
          "INSTRUIR msg_beber",
          "AGUARDAR_CONFIRMACAO 30s -> CONFIRMADO (4s)",
        ],
        estados: ["concluida"],
      },
    ],
  },
  {
    id: "ajuda",
    titulo: "Quando pedem ajuda",
    tema: "Condições",
    enunciado: [
      "Depois da espera, trate as três respostas possíveis.",
      "Com `AJUDA`, alerte com intensidade `BAIXA` e instrua com `msg_dica`. Se o tempo esgotar, alerte com `MEDIA`. Se a pessoa confirmar, não faça mais nada.",
    ],
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
    casos: [
      {
        titulo: "Confirma: nenhum alerta",
        respostas: "CONFIRMADO 2",
        naoContem: ["ALERTAR BAIXA", "ALERTAR MEDIA", "INSTRUIR msg_dica"],
        estados: ["concluida"],
      },
      {
        titulo: "Pede ajuda",
        respostas: "AJUDA 5",
        contem: ["ALERTAR BAIXA", "INSTRUIR msg_dica"],
        naoContem: ["ALERTAR MEDIA"],
        estados: ["concluida"],
      },
      {
        titulo: "Não responde",
        respostas: "TEMPO_ESGOTADO",
        contem: ["ALERTAR MEDIA"],
        naoContem: ["ALERTAR BAIXA", "INSTRUIR msg_dica"],
        estados: ["concluida"],
      },
    ],
  },
  {
    id: "camisa-blusa",
    titulo: "Camisa e blusa",
    tema: "Cancelamento",
    enunciado: [
      "Peça a camisa e depois a blusa, cada uma na sua etapa, esperando até `1min`.",
      "Se a pessoa pegar o objeto errado (qualquer resposta que não seja `CONFIRMADO`), alerte com `ALTA`, instrua com `msg_cancela`, registre `ROTINA_INCOMPLETA` e cancele a rotina com `retorna;`.",
    ],
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
    casos: [
      {
        titulo: "Pega as duas peças",
        respostas: "CONFIRMADO 3, CONFIRMADO 5",
        contem: ["ETAPA Pegar a camisa", "ETAPA Pegar a blusa", "AGUARDAR_CONFIRMACAO 60s -> CONFIRMADO (5s)"],
        naoContem: ["REGISTRAR ROTINA_INCOMPLETA"],
        estados: ["concluida"],
      },
      {
        titulo: "Erra a camisa",
        respostas: "AJUDA 4",
        contem: ["ALERTAR ALTA", "INSTRUIR msg_cancela", "REGISTRAR ROTINA_INCOMPLETA"],
        naoContem: ["ETAPA Pegar a blusa"],
        estados: ["cancelada"],
      },
      {
        titulo: "Pega a camisa, não pega a blusa",
        respostas: "CONFIRMADO 3, TEMPO_ESGOTADO",
        contem: ["ETAPA Pegar a blusa", "ALERTAR ALTA", "REGISTRAR ROTINA_INCOMPLETA"],
        estados: ["cancelada"],
      },
    ],
  },
  {
    id: "tres-tentativas",
    titulo: "Três tentativas",
    tema: "Laços",
    enunciado: [
      "Instrua com `msg_chave` e espere `20s`, até três vezes. Pare assim que a pessoa confirmar.",
      "Se ela não confirmar em nenhuma das três, registre `ETAPA_PULADA`.",
    ],
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
    casos: [
      {
        titulo: "Confirma na primeira",
        respostas: "CONFIRMADO 2",
        vezes: { "INSTRUIR msg_chave": 1 },
        naoContem: ["REGISTRAR ETAPA_PULADA"],
        estados: ["concluida"],
      },
      {
        titulo: "Confirma na terceira",
        respostas: "AJUDA 3, TEMPO_ESGOTADO, CONFIRMADO 4",
        vezes: { "INSTRUIR msg_chave": 3 },
        contem: ["AGUARDAR_CONFIRMACAO 20s -> CONFIRMADO (4s)"],
        naoContem: ["REGISTRAR ETAPA_PULADA"],
        estados: ["concluida"],
      },
      {
        titulo: "Nunca responde",
        respostas: "",
        vezes: { "INSTRUIR msg_chave": 3, "AGUARDAR_CONFIRMACAO 20s -> TEMPO_ESGOTADO (20s)": 3 },
        contem: ["REGISTRAR ETAPA_PULADA"],
        estados: ["concluida"],
      },
    ],
  },
  {
    id: "alerta-cresce",
    titulo: "Alerta que cresce",
    tema: "Funções",
    enunciado: [
      "Faça `alerta_para` devolver `BAIXA` na 1ª tentativa, `MEDIA` na 2ª e `ALTA` a partir da 3ª.",
      "Depois do laço, se a pessoa não tomou o remédio, registre `ROTINA_INCOMPLETA`.",
    ],
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
    casos: [
      {
        titulo: "Toma na segunda tentativa",
        respostas: "TEMPO_ESGOTADO, CONFIRMADO 8",
        contem: ["ALERTAR BAIXA"],
        naoContem: ["ALERTAR MEDIA", "REGISTRAR ROTINA_INCOMPLETA"],
        estados: ["concluida"],
      },
      {
        titulo: "Pede ajuda três vezes",
        respostas: "AJUDA 5, AJUDA 9, AJUDA 2",
        contem: ["ALERTAR BAIXA", "ALERTAR MEDIA", "ALERTAR ALTA", "REGISTRAR ROTINA_INCOMPLETA"],
        estados: ["concluida"],
      },
      {
        titulo: "Nunca responde",
        respostas: "",
        vezes: { "ALERTAR BAIXA": 1, "ALERTAR MEDIA": 1, "ALERTAR ALTA": 1, "REGISTRAR ROTINA_INCOMPLETA": 1 },
        estados: ["concluida"],
      },
    ],
  },
];

export interface ResultadoCaso {
  caso: Caso;
  ok: boolean;
  falhas: string[];
  respostas: string;
  registro: string[];
}

const LIMITE_RESPOSTAS = 50;

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
    return { caso, ok: false, falhas: ["O programa tem erros e não roda."], respostas, registro: [] };
  }
  const falhas: string[] = [];
  if (r.pausada !== null) {
    falhas.push(`A rotina continuou esperando depois de ${LIMITE_RESPOSTAS} respostas.`);
  }
  const linhas = r.registro;
  for (const l of caso.contem ?? []) {
    if (!linhas.includes(l)) falhas.push(`Faltou a linha “${l}”.`);
  }
  for (const l of caso.naoContem ?? []) {
    if (linhas.includes(l)) falhas.push(`Não deveria aparecer “${l}”.`);
  }
  for (const [l, n] of Object.entries(caso.vezes ?? {})) {
    const k = linhas.filter((x) => x === l).length;
    if (k !== n) falhas.push(`“${l}” deveria aparecer ${n}× e apareceu ${k}×.`);
  }
  if (caso.estados) {
    const obtidos = r.rotinas.map((x) => x.estado);
    if (obtidos.join("|") !== caso.estados.join("|")) {
      falhas.push(
        `Estado final esperado: ${caso.estados.join(", ")}. Obtido: ${obtidos.join(", ") || "nenhum"}.`,
      );
    }
  }
  return { caso, ok: falhas.length === 0, falhas, respostas, registro: linhas };
}
