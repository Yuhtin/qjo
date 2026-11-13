import "@fontsource-variable/bricolage-grotesque/standard.css";
import "@fontsource-variable/jetbrains-mono";
import "./estilo.css";

import { nextDiagnostic, openLintPanel } from "@codemirror/lint";
import { EditorView } from "@codemirror/view";
import * as qjo from "./qjo";
import { criarEditor, formatarDoc, type Editor, type EstadoAnalise } from "./editor";
import { EXERCICIOS, rodarCaso, type Exercicio, type ResultadoCaso } from "./exercicios";

// ---------- utilidades ----------

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, txt?: string) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (txt !== undefined) e.textContent = txt;
  return e;
}

/** Texto com `código` inline. */
function textoComCodigo(s: string): DocumentFragment {
  const f = document.createDocumentFragment();
  s.split(/(`[^`]+`)/g).forEach((p, i) => f.append(i % 2 ? el("code", undefined, p.slice(1, -1)) : p));
  return f;
}

const guardado = {
  ler(k: string): string | null {
    try {
      return localStorage.getItem(k);
    } catch {
      return null;
    }
  },
  gravar(k: string, v: string) {
    try {
      localStorage.setItem(k, v);
    } catch {
      /* armazenamento indisponível: segue sem persistir */
    }
  },
  apagar(k: string) {
    try {
      localStorage.removeItem(k);
    } catch {
      /* idem */
    }
  },
};

// O analisador precisa estar pronto antes do editor.
try {
  await qjo.carregar(`${import.meta.env.BASE_URL}qjo.wasm`);
} catch (e) {
  document.getElementById("cm")!.textContent = `Não foi possível carregar o analisador: ${(e as Error).message}`;
  throw e;
}

const mac = /Mac|iPhone|iPad/.test(navigator.userAgent);
for (const k of document.querySelectorAll<HTMLElement>("[data-atalho=enter]")) {
  k.textContent = mac ? "⌘↵" : "Ctrl ↵";
}

let toastTimer = 0;
function aviso(msg: string) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.add("visivel");
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => t.classList.remove("visivel"), 2200);
}

async function copiar(texto: string, ok: string) {
  try {
    await navigator.clipboard.writeText(texto);
    aviso(ok);
  } catch {
    aviso("Não foi possível copiar. Selecione o texto e copie à mão.");
  }
}

// ---------- arquivos: exemplos e exercícios ----------

interface Arquivo {
  id: string;
  titulo: string;
  descricao: string;
  original: string;
  exercicio?: Exercicio;
}

const brutos = import.meta.glob("../../examples/*.qjo", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const ORDEM = ["pegar_roupa", "escovar_dentes", "remedio"];

const exemplos: Arquivo[] = Object.entries(brutos)
  .map(([caminho, src]) => {
    const nome = caminho.split("/").pop()!.replace(/\.qjo$/, "");
    const rotina = /rotina\(\s*"([^"]+)"/.exec(src)?.[1] ?? nome.replace(/_/g, " ");
    const comentario = /^#\s*(.+)$/m.exec(src)?.[1] ?? "";
    return { id: `exemplo:${nome}`, titulo: rotina, descricao: comentario, original: src, nome };
  })
  .sort((a, b) => {
    const ia = ORDEM.indexOf(a.nome);
    const ib = ORDEM.indexOf(b.nome);
    return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib) || a.nome.localeCompare(b.nome);
  });

const exercicios: Arquivo[] = EXERCICIOS.map((e) => ({
  id: `exercicio:${e.id}`,
  titulo: e.titulo,
  descricao: e.tema,
  original: e.inicial,
  exercicio: e,
}));

const todos = [...exemplos, ...exercicios];

const CHAVE_TEXTO = (id: string) => `qjo.texto.${id}`;
const CHAVE_ATUAL = "qjo.atual";
const CHAVE_RESOLVIDOS = "qjo.resolvidos";

function resolvidos(): Set<string> {
  try {
    const v = JSON.parse(guardado.ler(CHAVE_RESOLVIDOS) ?? "[]");
    return new Set(Array.isArray(v) ? v.filter((x) => typeof x === "string") : []);
  } catch {
    return new Set();
  }
}

function marcarResolvido(id: string) {
  const s = resolvidos();
  s.add(id);
  guardado.gravar(CHAVE_RESOLVIDOS, JSON.stringify([...s]));
}

const textoSalvo = (a: Arquivo) => guardado.ler(CHAVE_TEXTO(a.id)) ?? a.original;

let atual: Arquivo = todos.find((a) => a.id === guardado.ler(CHAVE_ATUAL)) ?? exemplos[0] ?? exercicios[0];

// ---------- abas e layout ----------

const area = $("area");
const abas = [...document.querySelectorAll<HTMLButtonElement>(".aba")];

const visivel = (e: HTMLElement) => getComputedStyle(e).display !== "none";

function mostrarAba(nome: string) {
  area.dataset.aba = nome;
  for (const b of abas) {
    const sel = b.dataset.alvo === nome;
    b.setAttribute("aria-selected", String(sel));
    b.tabIndex = sel ? 0 : -1;
  }
  if (nome === "cpp") atualizarCpp();
  if (nome === "editor") requestAnimationFrame(() => editor.view.requestMeasure());
}

/** A aba ativa pode sumir quando a largura muda (por exemplo, "Código" no desktop). */
function normalizarAba() {
  const ativa = abas.find((b) => b.dataset.alvo === area.dataset.aba);
  if (!ativa || !visivel(ativa)) mostrarAba("sim");
}

for (const b of abas) b.addEventListener("click", () => mostrarAba(b.dataset.alvo!));
// Setas entre as abas visíveis.
$("area").querySelector(".abas")!.addEventListener("keydown", (ev) => {
  const e = ev as KeyboardEvent;
  if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
  const vis = abas.filter(visivel);
  const i = vis.findIndex((b) => b.dataset.alvo === area.dataset.aba);
  const prox = vis[(i + (e.key === "ArrowRight" ? 1 : vis.length - 1)) % vis.length];
  mostrarAba(prox.dataset.alvo!);
  prox.focus();
});
for (const q of ["(min-width: 50rem)", "(min-width: 75rem)"]) {
  matchMedia(q).addEventListener("change", normalizarAba);
}

/** Mostra um painel se ele estiver escondido atrás de uma aba. */
function revelar(painel: "sim" | "editor" | "cpp") {
  if (!visivel($(`p-${painel}`))) mostrarAba(painel);
}

// ---------- estado ----------

interface Resposta { r: "CONFIRMADO" | "AJUDA" | "TEMPO_ESGOTADO"; s: number }

const sim = {
  respostas: [] as Resposta[],
  /** Texto da última execução, para avisar quando o código muda. */
  rodado: null as string | null,
  limite: 0,
};

let cppSujo = true;
let cppTexto = "";

// ---------- editor ----------

let salvarTimer = 0;
let ultimaAnalise: EstadoAnalise | null = null;

const editor: Editor = criarEditor({
  pai: $("cm"),
  texto: textoSalvo(atual),
  aoAnalisar: aoAnalisar,
  aoMudar(texto) {
    clearTimeout(salvarTimer);
    const arq = atual;
    salvarTimer = window.setTimeout(() => {
      if (texto === arq.original) guardado.apagar(CHAVE_TEXTO(arq.id));
      else guardado.gravar(CHAVE_TEXTO(arq.id), texto);
    }, 250);
    atualizarEditado(texto);
    if (sim.rodado !== null) $("sim-mudou").hidden = texto === sim.rodado;
  },
  aoRodar: () => rodar(),
  aoFormatar: () => formatar(),
});

function atualizarEditado(texto: string) {
  const mudou = texto !== atual.original;
  $("editado").hidden = !mudou;
  $("restaurar").hidden = !mudou;
}

function aoAnalisar(est: EstadoAnalise) {
  ultimaAnalise = est;
  const n = est.analise.diagnosticos.length;
  const st = $<HTMLButtonElement>("status-diag");
  st.replaceChildren();
  st.classList.toggle("tem-erro", n > 0);
  if (n === 0) {
    st.append(el("span", "ponto ponto-ok"), "Sem erros");
    st.disabled = true;
  } else {
    const fase = est.analise.diagnosticos[0].fase;
    st.append(el("span", "ponto ponto-erro"), `${n} ${n === 1 ? "erro" : "erros"} · ${fase}`);
    st.disabled = false;
  }
  desenharEsboco(est.analise.esboco);
  cppSujo = true;
  if (visivel($("p-cpp"))) atualizarCpp();
}

// Abre a lista de erros, com o botão de correção de cada um (funciona sem mouse).
$("status-diag").addEventListener("click", () => {
  revelar("editor");
  nextDiagnostic(editor.view);
  openLintPanel(editor.view);
});

$("status-dica").textContent = `${mac ? "⌘" : "Ctrl"}+clique vai à definição · Shift+Alt+F formata`;

function formatar() {
  if (!formatarDoc(editor.view)) aviso("Corrija os erros antes de formatar.");
  editor.view.focus();
}

$("formatar").addEventListener("click", formatar);
$("rodar").addEventListener("click", () => rodar());
$("restaurar").addEventListener("click", () => {
  editor.trocar(atual.original);
  guardado.apagar(CHAVE_TEXTO(atual.id));
  atualizarEditado(atual.original);
  limparSim();
  aviso("Texto original restaurado.");
});

// ---------- biblioteca e esboço ----------

function desenharListas() {
  const feitos = resolvidos();
  const item = (a: Arquivo, extra?: (b: HTMLButtonElement) => void) => {
    const li = el("li");
    const b = el("button", "item");
    b.type = "button";
    b.dataset.id = a.id;
    if (a.id === atual.id) b.setAttribute("aria-current", "true");
    b.append(el("span", "item-titulo", a.titulo), el("span", "item-desc", a.descricao));
    extra?.(b);
    b.addEventListener("click", () => abrir(a));
    li.append(b);
    return li;
  };
  $("lista-exemplos").replaceChildren(...exemplos.map((a) => item(a)));
  $("lista-exercicios").replaceChildren(
    ...exercicios.map((a, i) =>
      item(a, (b) => {
        b.classList.add("item-ex");
        const feito = feitos.has(a.exercicio!.id);
        const marca = el("span", feito ? "item-num feito" : "item-num", feito ? "" : String(i + 1));
        marca.setAttribute("aria-hidden", "true");
        if (feito) b.append(el("span", "so-leitor", " (resolvido)"));
        if (feito) marca.innerHTML = '<svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M3.5 8.5l3 3 6-7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>';
        b.prepend(marca);
      }),
    ),
  );
  $("contador").textContent = `${[...feitos].filter((id) => EXERCICIOS.some((e) => e.id === id)).length}/${EXERCICIOS.length}`;
}

const ROTULO_ESPECIE: Record<qjo.EspecieEsboco, string> = {
  rotina: "rotina",
  etapa: "etapa",
  funcao: "função",
  enum: "enum",
  recurso: "recurso",
  horario: "horário",
};

function desenharEsboco(nos: qjo.NoEsboco[]) {
  const lista = (ns: qjo.NoEsboco[]): HTMLElement[] =>
    ns.map((n) => {
      const li = el("li");
      const b = el("button", `no no-${n.especie}`);
      b.type = "button";
      b.append(el("span", "no-nome", n.nome), el("span", "no-esp", ROTULO_ESPECIE[n.especie]));
      b.addEventListener("click", () => {
        revelar("editor");
        editor.view.dispatch({
          selection: { anchor: n.ini, head: n.fim },
          effects: EditorView.scrollIntoView(n.ini, { y: "center" }),
        });
        editor.view.focus();
      });
      li.append(b);
      if (n.filhos.length) {
        const sub = el("ul");
        sub.append(...lista(n.filhos));
        li.append(sub);
      }
      return li;
    });
  const raiz = $("esboco");
  if (!nos.length) raiz.replaceChildren(el("li", "esboco-vazio", "Nada declarado ainda."));
  else raiz.replaceChildren(...lista(nos));
}

function abrir(a: Arquivo) {
  if (a.id === atual.id) {
    revelar("editor");
    return;
  }
  // Salva já o texto do arquivo que sai, sem esperar o atraso.
  clearTimeout(salvarTimer);
  const saindo = editor.view.state.doc.toString();
  if (saindo === atual.original) guardado.apagar(CHAVE_TEXTO(atual.id));
  else guardado.gravar(CHAVE_TEXTO(atual.id), saindo);

  atual = a;
  guardado.gravar(CHAVE_ATUAL, a.id);
  const texto = textoSalvo(a);
  editor.trocar(texto);
  atualizarEditado(texto);
  desenharArquivo();
  desenharListas();
  limparSim();
  revelar("editor");
}

function desenharArquivo() {
  $("arquivo-nome").textContent = atual.exercicio ? `Exercício · ${atual.titulo}` : atual.titulo;
  const det = $<HTMLDetailsElement>("exercicio");
  const ex = atual.exercicio;
  det.hidden = !ex;
  $("casos").replaceChildren();
  $("ex-resumo").textContent = "";
  if (!ex) return;
  $("ex-tema").textContent = ex.tema;
  $("ex-titulo").textContent = ex.titulo;
  $("ex-resolvido").hidden = !resolvidos().has(ex.id);
  $("ex-texto").replaceChildren(...ex.enunciado.map((p) => {
    const e = el("p");
    e.append(textoComCodigo(p));
    return e;
  }));
  det.open = true;
}

// ---------- exercícios ----------

$("verificar").addEventListener("click", () => {
  const ex = atual.exercicio;
  if (!ex) return;
  const src = editor.view.state.doc.toString();
  const casos = $("casos");
  if (!qjo.analisar(src).ok) {
    casos.replaceChildren();
    $("ex-resumo").textContent = "Corrija os erros do código antes de verificar.";
    $("ex-resumo").dataset.estado = "erro";
    return;
  }
  const res = ex.casos.map((c) => rodarCaso(qjo.executar, src, c));
  const ok = res.filter((r) => r.ok).length;
  casos.replaceChildren(...res.map(desenharCaso));
  const tudo = ok === res.length;
  $("ex-resumo").textContent = tudo
    ? `Todos os ${res.length} casos passaram.`
    : `${ok} de ${res.length} ${res.length === 1 ? "caso passou" : "casos passaram"}.`;
  $("ex-resumo").dataset.estado = tudo ? "ok" : "falha";
  if (tudo) {
    const novo = !resolvidos().has(ex.id);
    marcarResolvido(ex.id);
    $("ex-resolvido").hidden = false;
    desenharListas();
    if (novo) aviso("Exercício resolvido.");
  }
});

function desenharCaso(r: ResultadoCaso): HTMLElement {
  const li = el("li", r.ok ? "caso caso-ok" : "caso caso-falha");
  const cab = el("div", "caso-cab");
  cab.append(
    el("span", "caso-icone", r.ok ? "✓" : "✕"),
    el("span", "caso-titulo", r.caso.titulo),
    el("code", "caso-resp", r.caso.respostas || "sem respostas"),
  );
  li.append(cab);
  if (!r.ok) {
    const ul = el("ul", "caso-falhas");
    ul.append(...r.falhas.map((f) => {
      // “linha do registro” vira código.
      const li2 = el("li");
      f.split(/“([^”]+)”/g).forEach((p, i) => li2.append(i % 2 ? el("code", undefined, p) : p));
      return li2;
    }));
    li.append(ul);
  }
  return li;
}

// ---------- simulador ----------

const textoRespostas = () =>
  sim.respostas.map((x) => (x.r === "TEMPO_ESGOTADO" ? x.r : `${x.r} ${x.s}`)).join(", ");

function estadoSim(tipo: "pronto" | "pausado" | "fim" | "erro", texto: string) {
  $("sim-estado").dataset.tipo = tipo;
  $("sim-estado-texto").textContent = texto;
}

function limparSim() {
  estadoSim("pronto", "Pronto para rodar");
  sim.respostas = [];
  sim.rodado = null;
  $("registro").replaceChildren();
  $("resumo").hidden = true;
  $("pergunta").hidden = true;
  $("sim-aviso").hidden = true;
  $("sim-mudou").hidden = true;
  $("sim-vazio").hidden = false;
  $<HTMLButtonElement>("reiniciar").disabled = true;
  desenharUsadas();
}

function rodar() {
  const src = editor.view.state.doc.toString();
  const a = qjo.analisar(src);
  revelar("sim");
  $("sim-vazio").hidden = true;
  $<HTMLButtonElement>("reiniciar").disabled = false;
  const avisoEl = $("sim-aviso");
  if (!a.ok) {
    const n = a.diagnosticos.length;
    avisoEl.replaceChildren(
      el("p", undefined, `O código tem ${n} ${n === 1 ? "erro" : "erros"}. Corrija antes de rodar.`),
    );
    const ver = el("button", "btn btn-mini", "Ver o primeiro erro");
    ver.type = "button";
    ver.addEventListener("click", () => {
      revelar("editor");
      nextDiagnostic(editor.view);
      editor.view.focus();
    });
    avisoEl.append(ver);
    avisoEl.hidden = false;
    estadoSim("erro", "Código com erros");
    $("registro").replaceChildren();
    $("resumo").hidden = true;
    $("pergunta").hidden = true;
    $("sim-mudou").hidden = true;
    return;
  }
  avisoEl.hidden = true;
  const r = qjo.executar(src, textoRespostas());
  sim.rodado = src;
  $("sim-mudou").hidden = true;
  if (!r.ok) {
    avisoEl.replaceChildren(el("p", undefined, r.erro ? `Não foi possível rodar: ${r.erro}.` : "Não foi possível rodar."));
    avisoEl.hidden = false;
    return;
  }
  desenharRegistro(r.registro);
  desenharUsadas();
  const perg = $("pergunta");
  if (r.pausada !== null) {
    sim.limite = r.pausada;
    $("resumo").hidden = true;
    $("pergunta-texto").replaceChildren(
      "A rotina espera uma confirmação, com limite de ",
      el("strong", undefined, `${r.pausada} s`),
      ".",
    );
    const inp = $<HTMLInputElement>("segundos");
    inp.max = String(r.pausada);
    inp.value = String(Math.min(Number(inp.value) || 0, r.pausada));
    $("segundos-max").textContent = `até ${r.pausada}`;
    perg.hidden = false;
    estadoSim("pausado", `Esperando a resposta ${sim.respostas.length + 1}`);
  } else {
    perg.hidden = true;
    desenharResumo(r.rotinas);
    estadoSim("fim", "Execução terminada");
  }
  // Mostra o fim do registro.
  const corpo = $("sim-corpo");
  requestAnimationFrame(() => corpo.scrollTo({ top: corpo.scrollHeight }));
}

type Tipo = "rotina" | "etapa" | "alerta" | "espera" | "evento" | "auto" | "fim-ok" | "fim-cancelada" | "fim-erro" | "cmd";

function tipoLinha(l: string): Tipo {
  if (l.startsWith("ROTINA ")) return "rotina";
  if (l.startsWith("ETAPA ")) return "etapa";
  if (l.endsWith("(fim da etapa)") || l.endsWith("(fim da rotina)")) return "auto";
  if (l === "FIM concluida") return "fim-ok";
  if (l === "FIM cancelada") return "fim-cancelada";
  if (l.startsWith("FIM ")) return "fim-erro";
  if (l.startsWith("ALERTAR ")) return "alerta";
  if (l.startsWith("AGUARDAR_CONFIRMACAO ")) return "espera";
  if (l.startsWith("REGISTRAR ")) return "evento";
  return "cmd";
}

function desenharRegistro(linhas: string[]) {
  let nivel = 0;
  const itens = linhas.map((l) => {
    const t = tipoLinha(l);
    if (t === "rotina") nivel = 0;
    else if (t === "etapa") nivel = 1;
    else if (t.startsWith("fim")) nivel = 0;
    const li = el("li", `ln ln-${t}`);
    li.style.setProperty("--nivel", String(t === "rotina" || t === "etapa" || t.startsWith("fim") ? nivel : nivel + 1));
    const esp = l.indexOf(" ");
    const cabeca = esp < 0 ? l : l.slice(0, esp);
    const resto = esp < 0 ? "" : l.slice(esp + 1);
    li.append(el("span", "ln-cmd", cabeca));
    if (t === "espera") {
      // AGUARDAR_CONFIRMACAO 60s -> CONFIRMADO (3s)
      const m = /^(\S+) -> (\S+) \((\S+)\)$/.exec(resto);
      if (m) {
        li.append(" ", el("span", "ln-arg", m[1]), el("span", "ln-seta", " → "),
          el("span", `chip chip-${m[2].toLowerCase()}`, m[2]), el("span", "ln-tempo", ` em ${m[3]}`));
      } else li.append(" ", el("span", "ln-arg", resto));
    } else if (t === "auto") {
      const m = /^(.*) (\(fim da (?:etapa|rotina)\))$/.exec(resto);
      li.append(" ", el("span", "ln-arg", m ? m[1] : resto));
      if (m) li.append(" ", el("span", "ln-nota", m[2]));
    } else if (resto) {
      li.append(" ", el("span", t === "rotina" || t === "etapa" ? "ln-nome" : "ln-arg", resto));
    }
    if (t === "alerta") li.dataset.nivel = resto.toLowerCase();
    return li;
  });
  $("registro").replaceChildren(...itens);
}

function desenharResumo(rotinas: qjo.RotinaFinal[]) {
  const box = $("resumo");
  const lista = el("ul", "resumo-lista");
  for (const r of rotinas) {
    const li = el("li");
    const tipo = r.estado === "concluida" ? "ok" : r.estado === "cancelada" ? "cancelada" : "erro";
    const rotulo = r.estado === "concluida" ? "concluída" : r.estado === "cancelada" ? "cancelada" : "erro";
    li.append(el("span", "resumo-nome", r.nome), el("span", `selo selo-${tipo}`, rotulo));
    if (tipo === "erro") li.append(el("span", "resumo-motivo", r.estado.replace(/^erro:\s*/, "")));
    lista.append(li);
  }
  box.replaceChildren(el("h3", "rotulo", "Fim da execução"), lista);
  box.hidden = false;
}

function desenharUsadas() {
  atualizarRepetir();
  const box = $("usadas");
  box.hidden = sim.respostas.length === 0;
  $("usadas-lista").replaceChildren(
    ...sim.respostas.map((x) =>
      el("li", `usada usada-${x.r.toLowerCase()}`, x.r === "TEMPO_ESGOTADO" ? x.r : `${x.r} ${x.s}s`),
    ),
  );
}

$("pergunta").addEventListener("submit", (ev) => {
  ev.preventDefault();
  const botao = (ev as SubmitEvent).submitter as HTMLButtonElement | null;
  const r = (botao?.value ?? "CONFIRMADO") as Resposta["r"];
  const inp = $<HTMLInputElement>("segundos");
  let s = Math.floor(Number(inp.value));
  if (!Number.isFinite(s) || s < 0) s = 0;
  if (s > sim.limite) s = sim.limite;
  inp.value = String(s);
  sim.respostas.push({ r, s });
  rodar();
  // Mantém o foco no formulário para responder em sequência pelo teclado.
  if (!$("pergunta").hidden) botao?.focus();
});

$("segundos").addEventListener("change", () => {
  const inp = $<HTMLInputElement>("segundos");
  const s = Math.max(0, Math.min(sim.limite, Math.floor(Number(inp.value) || 0)));
  inp.value = String(s);
});

$("desfazer").addEventListener("click", () => {
  sim.respostas.pop();
  rodar();
});

$("reiniciar").addEventListener("click", () => {
  sim.respostas = [];
  rodar();
});
$("rodar-vazio").addEventListener("click", () => rodar());
$("rodar-de-novo").addEventListener("click", () => rodar());

// ---------- C++ ----------


function atualizarCpp() {
  if (!cppSujo) return;
  cppSujo = false;
  cppTexto = qjo.cpp(editor.view.state.doc.toString());
  const pre = $("cpp-codigo");
  const ok = cppTexto !== "";
  $<HTMLButtonElement>("cpp-copiar").disabled = !ok;
  $<HTMLButtonElement>("cpp-baixar").disabled = !ok;
  if (!ok) {
    pre.classList.add("cpp-vazio");
    const n = ultimaAnalise?.analise.diagnosticos.length ?? 0;
    pre.textContent = `O código tem ${n || "alguns"} ${n === 1 ? "erro" : "erros"}. Corrija para gerar o C++.`;
  } else {
    pre.classList.remove("cpp-vazio");
    pre.textContent = cppTexto;
  }
  atualizarRepetir();
}

/** A linha "printf … | ./rotina" com as respostas do simulador. */
function atualizarRepetir() {
  const rep = $("cpp-repetir");
  if (cppTexto && sim.respostas.length) {
    const linhas = sim.respostas.map((x) => (x.r === "TEMPO_ESGOTADO" ? x.r : `${x.r} ${x.s}`)).join("\\n");
    rep.replaceChildren("Para repetir as respostas do simulador: ", el("code", undefined, `printf '${linhas}\\n' | ./rotina`));
    rep.hidden = false;
  } else rep.hidden = true;
}

$("cpp-copiar").addEventListener("click", () => copiar(cppTexto, "C++ copiado."));
$("cpp-copiar-cmd").addEventListener("click", () => copiar($("cpp-comando").textContent ?? "", "Comando copiado."));
$("cpp-baixar").addEventListener("click", () => {
  const url = URL.createObjectURL(new Blob([cppTexto], { type: "text/x-c++src" }));
  const a = el("a");
  a.href = url;
  a.download = "rotina.cpp";
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
});

// ---------- início ----------

desenharArquivo();
desenharListas();
atualizarEditado(editor.view.state.doc.toString());
// No celular, a primeira aba é o código.
const abaCodigo = abas.find((b) => b.dataset.alvo === "editor")!;
if (visivel(abaCodigo)) mostrarAba("editor");
normalizarAba();
