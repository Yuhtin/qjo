// Interface do site: o mesmo desenho e o mesmo comportamento da QueijoCode, sobre o núcleo
// em Rust compilado para WebAssembly (qjo.wasm). Rascunho = editor | saída; exercício =
// descrição | editor | saída.

import "./estilo.css";

import * as qjo from "./qjo";
import { criarEditor, formatarDoc, realcarTrecho, type EstadoAnalise } from "./editor";
import { EXERCICIOS, rodarCaso, type Exercicio, type ResultadoCaso } from "./exercicios";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const esc = (s: unknown) =>
  String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const inl = (s: string) => esc(s).replace(/`([^`]+)`/g, "<code>$1</code>");

function store(k: string, v?: string): string | null {
  try {
    if (v === undefined) return localStorage.getItem(k);
    localStorage.setItem(k, v);
  } catch {
    /* armazenamento indisponível: segue sem guardar */
  }
  return null;
}

try {
  await qjo.carregar(`${import.meta.env.BASE_URL}qjo.wasm`);
} catch (e) {
  $("diags").innerHTML = `<div class="blocked">Não foi possível carregar o compilador: ${esc((e as Error).message)}</div>`;
  throw e;
}

// ---------- arquivos ----------

interface Arquivo {
  id: string;
  titulo: string;
  arquivo: string;
  inicio: string;
  ex?: Exercicio;
}

const RASCUNHO: Arquivo = {
  id: "rascunho",
  titulo: "Rascunho livre",
  arquivo: "rascunho.qjo",
  inicio:
    '# Rascunho livre: escreva qualquer rotina e use o Simulador.\ndata horario = data"09:00:00";\nmensagem oi = "Bom dia!";\n\nrotina("Teste", horario) {\n    etapa("Cumprimentar") {\n        INSTRUIR(oi);\n        resultado r = AGUARDAR_CONFIRMACAO(30s);\n        q? (r == CONFIRMADO) {\n            REGISTRAR(CONFIRMACAO_RAPIDA);\n        }\n    }\n}\n',
};
const ARQ_EX: Arquivo[] = EXERCICIOS.map((e) => ({ id: e.id, titulo: e.titulo, arquivo: e.arquivo, inicio: e.inicial, ex: e }));

const nivelClasse = (n: string) => "nivel-" + n.normalize("NFD").replace(/[\u0300-\u036f]/g, "");

// ---------- estado ----------

interface Comp {
  src: string;
  analise: qjo.Analise;
  cpp: string;
}

interface Sim {
  src: string;
  respostas: string[];
  log: string[];
  /** Limite em segundos da espera em aberto, ou null. */
  esperando: number | null;
  fim: boolean;
  erro: string | null;
}

let atual: Arquivo = RASCUNHO;
let comp: Comp;
let resultadosTestes: { src: string; res: ResultadoCaso[] } | null = null;
let sim: Sim | null = null;
let spoiler: "fechado" | "confirmar" | "aberto" = "fechado";

const modoExercicio = () => atual !== RASCUNHO;
const salvarCodigo = () => store("qjo:code:" + atual.id, editor.texto());

function resolvidos(): Record<string, 1> {
  try {
    return JSON.parse(store("qjo:solved") || "{}") || {};
  } catch {
    return {};
  }
}
function marcarResolvido(id: string) {
  const s = resolvidos();
  s[id] = 1;
  store("qjo:solved", JSON.stringify(s));
}

const editor = criarEditor({
  pai: $("editor"),
  texto: store("qjo:code:" + RASCUNHO.id) || RASCUNHO.inicio,
  aoMudar: recompilar,
  aoRodar: atalhoRodar,
  aoFormatar: formatar,
});

function abrir(p: Arquivo) {
  atual = p;
  resultadosTestes = null;
  sim = null;
  spoiler = "fechado";
  const ex = modoExercicio();
  document.body.classList.toggle("exercicio", ex);
  $("crumb").hidden = !ex;
  $("crumb-where").textContent = ex ? p.titulo : "";
  $("b-sair").hidden = !ex;
  $("b-treinar-t").textContent = ex ? "Trocar exercício" : "Treinar exercícios";
  $("t-testes").hidden = !ex;
  $("b-test").hidden = !ex;
  $("b-sim").className = ex ? "btn" : "btn primary";
  $("fname").textContent = p.arquivo;
  selecionarAba("desc", "descricao");
  selecionarAba("out", ex ? "testes" : "sim");
  renderDescricao();
  renderSolucao();
  editor.trocar(store("qjo:code:" + p.id) || p.inicio);
  renderSim();
}

// ---------- modal ----------

function abrirModal() {
  const s = resolvidos();
  const feitos = EXERCICIOS.filter((e) => s[e.id]).length;
  $("m-bar").style.width = (feitos / EXERCICIOS.length) * 100 + "%";
  $("m-txt").textContent = feitos + " de " + EXERCICIOS.length + " resolvidos";
  $("m-grid").innerHTML = EXERCICIOS.map(
    (e, k) =>
      '<button class="ex-card" type="button" data-ex="' + e.id + '"><span class="h"><span class="num">' + (k + 1) +
      '</span><span class="t">' + esc(e.titulo) + "</span>" +
      (s[e.id] ? '<span class="chip solved">resolvido</span>' : '<span class="chip ' + nivelClasse(e.nivel) + '">' + e.nivel + "</span>") +
      '</span><p class="r">' + esc(e.resumo) + '</p><span class="f">' +
      e.conceitos.map((c) => "<code>" + esc(c) + "</code>").join("") +
      '<span class="n">' + e.casos.length + " testes</span></span></button>",
  ).join("");
  const m = $<HTMLDialogElement>("modal");
  if (typeof m.showModal === "function") m.showModal();
  else m.setAttribute("open", "");
}
function fecharModal() {
  const m = $<HTMLDialogElement>("modal");
  if (typeof m.close === "function") m.close();
  else m.removeAttribute("open");
}

// ---------- descrição e solução ----------

/** "CONFIRMADO 3, AJUDA 5" -> CONFIRMADO em 3s → AJUDA em 5s. */
function fmtEntrada(respostas: string): string {
  const itens = respostas.split(",").map((x) => x.trim()).filter(Boolean);
  if (!itens.length) return "<b>nenhuma resposta</b> (todas as esperas esgotam)";
  return itens
    .map((x) => {
      const [r, t] = x.split(/\s+/);
      return "<b>" + esc(r) + "</b>" + (r === "TEMPO_ESGOTADO" || t === undefined ? "" : " em " + esc(t) + "s");
    })
    .join(" → ");
}

function renderDescricao() {
  const p = atual.ex;
  if (!p) return;
  const s = resolvidos();
  $("p-descricao").innerHTML =
    '<div class="p-head"><h1>' + esc(p.titulo) + '</h1><div class="p-meta"><span class="chip ' + nivelClasse(p.nivel) + '">' + p.nivel + "</span>" +
    (s[p.id] ? '<span class="chip solved">resolvido</span>' : "") + "</div></div>" +
    '<div class="prose">' + p.enunciado.map((t) => "<p>" + inl(t) + "</p>").join("") + "</div>" +
    '<div class="sec"><span class="lbl">Dicas</span><ul>' + p.dicas.map((d) => "<li>" + inl(d) + "</li>").join("") + "</ul></div>" +
    '<div class="sec"><span class="lbl">Casos de teste</span><ul class="ex-list">' +
    p.casos.map((c) => '<li><span class="n">' + esc(c.titulo) + '</span><span class="e">' + fmtEntrada(c.respostas) + "</span></li>").join("") +
    "</ul></div>" +
    '<p class="note">O dispositivo é simulado: cada teste responde às esperas por confirmação na ordem da entrada. Respostas que faltarem viram tempo esgotado.</p>';
}

const ICON_EYE =
  '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 3l18 18"/><path d="M10.6 10.6a2 2 0 0 0 2.8 2.8"/><path d="M9.9 5.1A9.8 9.8 0 0 1 12 5c5 0 9 4.5 10 7a13 13 0 0 1-3.2 4.2M6.6 6.6A13 13 0 0 0 2 12c1 2.5 5 7 10 7a9.6 9.6 0 0 0 4.3-1"/></svg>';

function renderSolucao() {
  const p = $("p-solucao");
  const ex = atual.ex;
  if (!ex) {
    p.innerHTML = "";
    return;
  }
  if (spoiler !== "aberto") {
    const linhas = [62, 48, 70, 40, 0, 55, 66, 44, 72, 38, 58, 50, 68, 30, 46, 60]
      .map((w) => '<span style="width:' + (w || 4) + "%" + (w ? "" : ";opacity:0") + '"></span>')
      .join("");
    const card =
      spoiler === "fechado"
        ? '<span class="ic">' + ICON_EYE + '</span><h3>Alerta de spoiler</h3><p>A solução mostra o código completo deste exercício. Tente resolver antes de olhar.</p><div class="row"><button class="btn sm" type="button" data-act="spoiler-pedir">Revelar solução</button></div>'
        : '<span class="ic">' + ICON_EYE + '</span><h3>Tem certeza?</h3><p>Depois de ver, não dá para desver.</p><div class="row"><button class="btn ghost sm" type="button" data-act="spoiler-voltar">Ainda não</button><button class="btn sm warn" type="button" data-act="spoiler-abrir">Sim, mostrar</button></div>';
    p.innerHTML =
      '<div class="spoiler"><div class="ghost-code" aria-hidden="true">' + linhas +
      '</div><div class="gate"><div class="gate-card" role="group" aria-label="Alerta de spoiler">' + card + "</div></div></div>";
    const foco = p.querySelector<HTMLElement>('[data-act="spoiler-pedir"], [data-act="spoiler-voltar"]');
    if (foco && document.activeElement?.closest?.("#p-solucao")) foco.focus();
    return;
  }
  p.innerHTML =
    '<div class="toolbar"><span class="fname">' + esc(ex.arquivo) +
    '</span><button class="btn ghost sm" type="button" data-act="spoiler-esconder">Esconder</button><button class="btn sm" type="button" data-act="usar-solucao">Copiar para o editor</button></div>' +
    '<pre class="code" tabindex="0" id="sol-code"></pre>' +
    '<div class="sec"><span class="lbl">Por que funciona</span><p class="hint">' + inl(ex.explicacao) + "</p></div>";
  realcarTrecho(ex.solucao, $("sol-code"));
}

// ---------- compilação ----------

function recompilar(e: EstadoAnalise) {
  const a = e.analise;
  comp = { src: e.texto, analise: a, cpp: a.ok ? qjo.cpp(e.texto) : "" };
  renderDiags();
  renderTokens();
  renderCpp();
  $<HTMLButtonElement>("b-cpp").disabled = !comp.cpp;
  if (resultadosTestes && resultadosTestes.src !== comp.src) resultadosTestes = null;
  renderTestes();
  if (sim) renderSim();
  salvarCodigo();
}

const erros = () => comp.analise.diagnosticos;

const FASE_LABEL: Record<string, string> = {
  "léxico": "erro de escrita (léxico)",
  sintaxe: "erro de sintaxe",
  "semântica": "erro de tipo ou nome",
};

function renderDiags() {
  if (!erros().length) {
    $("diags").innerHTML = '<div class="empty"><span aria-hidden="true">✓</span> Sem erros. Passou no léxico, na sintaxe e na checagem de tipos.</div>';
    return;
  }
  $("diags").innerHTML = erros()
    .map(
      (e, k) =>
        '<div class="diag"><button class="pos" type="button" data-go="' + k + '" aria-label="Ir para linha ' + e.linha + ", coluna " + e.coluna + '">' +
        e.linha + ":" + e.coluna + "</button>" +
        '<div><span class="code">' + e.codigo + "</span>" + esc(e.mensagem) + '<div class="phase">' + (FASE_LABEL[e.fase] ?? e.fase) + "</div></div>" +
        (e.sugestao ? '<button class="btn sm" type="button" data-fix="' + k + '">' + esc(e.sugestao.titulo) + "</button>" : "<span></span>") +
        "</div>",
    )
    .join("");
}

function formatar() {
  if (formatarDoc(editor.view)) toast("Código formatado");
  else toast("Corrija os erros antes de formatar");
  editor.view.focus();
}

// ---------- testes ----------

function rodarTestes() {
  const ex = atual.ex;
  if (!ex) return;
  selecionarAba("out", "testes");
  if (erros().length) {
    resultadosTestes = null;
    renderTestes();
    return;
  }
  const res = ex.casos.map((c) => rodarCaso(qjo.executar, comp.src, c));
  resultadosTestes = { src: comp.src, res };
  if (res.every((x) => x.ok)) {
    marcarResolvido(ex.id);
    renderDescricao();
    toast("Todos os testes passaram");
  }
  renderTestes();
}

function bloqueado(msg: string) {
  const n = erros().length;
  return '<div class="blocked">' + esc(msg) + " Corrija " + (n === 1 ? "o erro" : "os " + n + " erros") + " abaixo do editor.</div>";
}

function renderTestes() {
  const p = $("p-testes");
  const b = $("badge-testes");
  const ex = atual.ex;
  if (!ex) {
    p.innerHTML = "";
    return;
  }
  if (erros().length) {
    p.innerHTML = bloqueado("Os testes rodam depois que o programa compila.") + casosHtml(ex, null);
    b.textContent = "–";
    b.className = "badge";
    return;
  }
  if (!resultadosTestes) {
    p.innerHTML = '<p class="hint">Compilou. Clique em <b>Rodar testes</b> para conferir a lógica.</p>' + casosHtml(ex, null);
    b.textContent = String(ex.casos.length);
    b.className = "badge";
    return;
  }
  const res = resultadosTestes.res;
  const ok = res.filter((x) => x.ok).length;
  const win = ok === res.length;
  b.textContent = ok + "/" + res.length;
  b.className = "badge " + (win ? "ok" : "bad");
  p.innerHTML =
    '<div class="summary' + (win ? " win" : "") + '"><span class="big">' + ok + "/" + res.length + "</span><span>" +
    (win ? "testes passaram. Exercício resolvido." : "testes passaram.") + "</span></div>" + casosHtml(ex, res);
}

function casosHtml(ex: Exercicio, res: ResultadoCaso[] | null) {
  return (
    '<div class="cases">' +
    ex.casos
      .map((c, k) => {
        const r = res?.[k];
        const pill = r
          ? r.ok ? '<span class="pill ok">passou</span>' : '<span class="pill bad">falhou</span>'
          : '<span class="pill idle">não rodou</span>';
        let h = '<div class="case"><div class="case-h"><span class="t">' + esc(c.titulo) + "</span>" + pill + '</div><div class="input-line">Entrada: ' + fmtEntrada(c.respostas) + "</div>";
        if (r) {
          if (r.erro) h += '<div class="blocked">Erro de execução: ' + esc(r.erro) + "</div>";
          if (r.conferencias.length) {
            h += '<table class="checks"><tr><th>Confere</th><th>Esperado</th><th>Obtido</th><th></th></tr>' +
              r.conferencias
                .map(
                  (x) =>
                    "<tr><td>" + esc(x.rotulo) + "</td><td>" + esc(x.esperado) + "</td><td>" + esc(x.obtido) +
                    '</td><td class="v ' + (x.ok ? 'ok">ok' : 'bad">difere') + "</td></tr>",
                )
                .join("") + "</table>";
          }
          h += '<details><summary>Ver o que o dispositivo fez</summary><pre class="trace">' + esc(r.registro.join("\n")) + "</pre></details>";
        }
        return h + "</div>";
      })
      .join("") +
    "</div>"
  );
}

// ---------- simulador ----------

/** Descrição de cada objeto e mensagem declarados no topo: `objeto x = "..."`. */
function descricoes(src: string): Map<string, string> {
  const m = new Map<string, string>();
  const t = qjo.analisar(src).tokens.filter(([c]) => c !== "comentario");
  const txt = (i: number) => src.slice(t[i][1], t[i][2]);
  for (let i = 0; i + 3 < t.length; i++) {
    if (t[i][0] !== "tipo" || !/^(objeto|mensagem)$/.test(txt(i)) || txt(i + 2) !== "=" || t[i + 3][0] !== "texto") continue;
    const lit = txt(i + 3).slice(1, -1).replace(/\\(["\\n])/g, (_, c) => (c === "n" ? "\n" : c));
    m.set(txt(i + 1), lit);
  }
  return m;
}

type Icone = "" | "wait" | "reg" | "end" | "err" | "step";

function acao(linha: string, desc: Map<string, string>): { html: string; ic: Icone; etapa?: boolean } {
  let m: RegExpExecArray | null;
  const nome = (o: string) => "<b>" + esc(o) + "</b>";
  if ((m = /^ROTINA (.*)$/.exec(linha))) return { html: "Começa a rotina “" + esc(m[1]) + "”", ic: "step" };
  if ((m = /^ETAPA (.*)$/.exec(linha))) return { html: "Etapa “" + esc(m[1]) + "”", ic: "step", etapa: true };
  if ((m = /^INDICAR (\S+)$/.exec(linha))) {
    const d = desc.get(m[1]);
    return { html: "Indica " + nome(m[1]) + (d ? ": " + esc(d) : ""), ic: "" };
  }
  if ((m = /^PARAR_INDICACAO (\S+)(?: \(fim da (etapa|rotina)\))?$/.exec(linha))) {
    return { html: "Para de indicar " + nome(m[1]) + (m[2] ? " (a " + m[2] + " acabou)" : ""), ic: "" };
  }
  if ((m = /^INSTRUIR (\S+)$/.exec(linha))) {
    const d = desc.get(m[1]);
    return { html: d ? "Mostra: “" + esc(d) + "”" : "Mostra " + nome(m[1]), ic: "" };
  }
  if ((m = /^AGUARDAR_CONFIRMACAO (\d+)s -> (\S+) \((\d+)s\)$/.exec(linha))) {
    const [, limite, r, t] = m;
    return { html: "Resposta: <b>" + r + "</b>" + (r === "TEMPO_ESGOTADO" ? " depois de " : " em ") + t + "s (limite " + limite + "s)", ic: "wait" };
  }
  if ((m = /^ALERTAR (\S+)$/.exec(linha))) return { html: "Alerta com intensidade <b>" + esc(m[1]) + "</b>", ic: "" };
  if ((m = /^REGISTRAR (\S+)$/.exec(linha))) return { html: "Registra para o terapeuta: <b>" + esc(m[1]) + "</b>", ic: "reg" };
  if ((m = /^ESPERAR (\d+)s$/.exec(linha))) return { html: "Pausa de " + m[1] + "s", ic: "" };
  if (linha === "FIM concluida") return { html: "Rotina chegou ao fim", ic: "end" };
  if (linha === "FIM cancelada") return { html: "Rotina encerrada com retorna;", ic: "end" };
  if ((m = /^FIM erro:\s*(.*)$/.exec(linha))) return { html: "Erro de execução: " + esc(m[1]), ic: "err" };
  return { html: esc(linha), ic: "" };
}

function iniciarSim() {
  selecionarAba("out", "sim");
  if (erros().length) {
    sim = null;
    renderSim();
    return;
  }
  sim = { src: comp.src, respostas: [], log: [], esperando: null, fim: false, erro: null };
  avancarSim();
}

/** A execução é determinística: roda de novo do começo com todas as respostas dadas até agora. */
function avancarSim() {
  if (!sim) return;
  const r = qjo.executar(sim.src, sim.respostas.join(", "));
  if (r.ok) {
    sim.log = r.registro;
    sim.esperando = r.pausada;
    sim.fim = r.pausada === null;
    sim.erro = null;
  } else {
    sim.esperando = null;
    sim.fim = true;
    sim.erro = r.erro ?? "não foi possível rodar";
  }
  renderSim();
  const p = $("p-sim");
  p.scrollTop = p.scrollHeight;
  p.querySelector<HTMLElement>('[data-resp="CONFIRMADO"]')?.focus();
}

function renderSim() {
  const p = $("p-sim");
  const comErro = !!comp && erros().length > 0;
  if (!sim) {
    p.innerHTML =
      (comErro ? bloqueado("O simulador roda depois que o programa compila.") : "") +
      '<p class="hint">Roda a rotina passo a passo. Quando ela esperar uma confirmação, você responde no lugar da pessoa.</p>' +
      '<div><button class="btn primary" type="button" data-act="sim-start"' + (comErro ? " disabled" : "") + ">Começar simulação</button></div>";
    return;
  }
  const desc = descricoes(sim.src);
  let h =
    '<div class="toolbar"><span class="hint" style="margin-inline-end:auto">' +
    (sim.fim ? "Simulação terminada." : sim.esperando !== null ? "Esperando sua resposta." : "") +
    (sim.src !== comp.src ? " O código mudou desde o começo." : "") +
    '</span><button class="btn sm" type="button" data-act="sim-start"' + (comErro ? " disabled" : "") + ">Recomeçar</button></div>";
  if (sim.erro) h += '<div class="blocked">' + esc(sim.erro) + "</div>";
  h +=
    '<ul class="sim-log">' +
    sim.log
      .map((l) => {
        const x = acao(l, desc);
        return '<li class="' + (x.etapa ? "etapa" : "") + '"><span class="ic ' + x.ic + '"></span><span>' + x.html + '<span class="raw">' + esc(l) + "</span></span></li>";
      })
      .join("") +
    "</ul>";
  if (sim.esperando !== null) {
    h +=
      '<div class="waitbox"><div class="q">O dispositivo espera confirmação (limite ' + sim.esperando + "s). O que a pessoa faz?</div>" +
      '<div class="row"><label for="sim-t">Responde em <input id="sim-t" type="number" min="0" max="' + sim.esperando + '" value="3" inputmode="numeric"> s</label></div>' +
      '<div class="row"><button class="btn primary sm" type="button" data-resp="CONFIRMADO">Confirma</button><button class="btn sm" type="button" data-resp="AJUDA">Pede ajuda</button><button class="btn sm" type="button" data-resp="TEMPO_ESGOTADO">Não responde</button></div></div>';
  }
  p.innerHTML = h;
}

// ---------- tokens ----------

function renderTokens() {
  const toks = qjo.tokens(comp.src);
  const lexErr = erros().filter((e) => e.fase === "léxico").length;
  $("p-tok").innerHTML =
    '<p class="hint">Saída do analisador léxico: ' + toks.length + " tokens" +
    (lexErr ? " e " + lexErr + " erro" + (lexErr > 1 ? "s" : "") + " léxico" + (lexErr > 1 ? "s" : "") + " (os tokens válidos ao redor continuam)" : "") +
    ". Mesmo formato da CLI (<code>qjo tokens --json</code>): um JSON por linha.</p>" +
    '<div><button class="btn sm" type="button" data-act="copy-jsonl">Copiar como JSON Lines</button></div>' +
    '<div style="overflow-x:auto"><table class="tok-table"><tr><th>linha</th><th>col</th><th>token</th><th>lexema</th></tr>' +
    toks.map((t) => "<tr><td>" + t.linha + "</td><td>" + t.coluna + '</td><td class="tk">' + t.token + "</td><td>" + esc(t.lexema) + "</td></tr>").join("") +
    "</table></div>";
}

// ---------- C++ ----------

function renderCpp() {
  const p = $("p-cpp");
  if (!comp.cpp) {
    p.innerHTML = bloqueado("O C++ é gerado depois que o programa compila.");
    return;
  }
  const nome = atual.arquivo.replace(/\.qjo$/, "");
  p.innerHTML =
    '<div class="toolbar"><span class="fname">' + esc(nome) + '.cpp</span><button class="btn ghost sm" type="button" data-act="cpp-baixar">Baixar</button><button class="btn sm" type="button" data-act="cpp-copiar">Copiar</button></div>' +
    '<p class="hint">Compile com <code>g++ -std=c++17 -o ' + esc(nome) + " " + esc(nome) + ".cpp</code>. O programa lê as respostas da entrada padrão, uma por linha, como <code>CONFIRMADO 3</code>; quando a entrada acaba, cada espera vira <code>TEMPO_ESGOTADO</code>.</p>" +
    '<pre class="code" tabindex="0">' + esc(comp.cpp) + "</pre>";
}

function baixarCpp() {
  const url = URL.createObjectURL(new Blob([comp.cpp], { type: "text/x-c++src" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = atual.arquivo.replace(/\.qjo$/, "") + ".cpp";
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

// ---------- utilidades ----------

let toastT = 0;
function toast(msg: string) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.add("show");
  clearTimeout(toastT);
  toastT = window.setTimeout(() => t.classList.remove("show"), 1800);
}

function copiar(txt: string, okMsg?: string) {
  try {
    navigator.clipboard.writeText(txt).then(
      () => toast(okMsg || "Copiado"),
      () => toast("Não deu para copiar neste navegador"),
    );
  } catch {
    toast("Não deu para copiar neste navegador");
  }
}

function selecionarAba(grupo: string, nome: string) {
  const g = document.querySelector<HTMLElement>('.tabs[data-group="' + grupo + '"]')!;
  for (const t of g.querySelectorAll<HTMLElement>(".tab")) {
    const sel = t.dataset.tab === nome;
    t.setAttribute("aria-selected", String(sel));
    $("p-" + t.dataset.tab).hidden = !sel;
  }
}

function atalhoRodar() {
  if (modoExercicio()) rodarTestes();
  else iniciarSim();
}

// ---------- eventos ----------

$("diags").addEventListener("click", (e) => {
  const alvo = e.target as HTMLElement;
  const g = alvo.closest<HTMLElement>("[data-go]");
  const f = alvo.closest<HTMLElement>("[data-fix]");
  if (g) {
    const d = erros()[+g.dataset.go!];
    editor.irPara(d.ini, d.fim);
  }
  if (f) {
    const d = erros()[+f.dataset.fix!];
    if (d.sugestao) {
      editor.editar(d.sugestao.edicoes);
      toast("Correção aplicada");
    }
  }
});
for (const g of document.querySelectorAll<HTMLElement>(".tabs")) {
  g.addEventListener("click", (e) => {
    const t = (e.target as HTMLElement).closest<HTMLElement>(".tab");
    if (t) selecionarAba(g.dataset.group!, t.dataset.tab!);
  });
}
$("b-test").addEventListener("click", rodarTestes);
$("b-sim").addEventListener("click", iniciarSim);
$("b-treinar").addEventListener("click", abrirModal);
$("b-sair").addEventListener("click", () => abrir(RASCUNHO));
$("b-cpp").addEventListener("click", () => {
  if (comp.cpp) copiar(comp.cpp, "C++ copiado. Compile com g++ -std=c++17");
});
$("b-reset").addEventListener("click", () => {
  editor.trocar(atual.inicio);
  toast("Código inicial restaurado");
});
$("modal").addEventListener("click", (e) => {
  if (e.target === e.currentTarget) {
    fecharModal();
    return;
  }
  const c = (e.target as HTMLElement).closest<HTMLElement>("[data-ex]");
  if (c) {
    const ex = ARQ_EX.find((x) => x.id === c.dataset.ex);
    fecharModal();
    if (ex) abrir(ex);
  }
});
document.addEventListener("click", (e) => {
  const alvo = e.target as HTMLElement;
  const a = alvo.closest<HTMLElement>("[data-act]");
  if (a) {
    const act = a.dataset.act;
    if (act === "sim-start") iniciarSim();
    if (act === "fechar-modal") fecharModal();
    if (act === "spoiler-pedir") (spoiler = "confirmar"), renderSolucao();
    if (act === "spoiler-voltar") (spoiler = "fechado"), renderSolucao();
    if (act === "spoiler-abrir") (spoiler = "aberto"), renderSolucao();
    if (act === "spoiler-esconder") (spoiler = "fechado"), renderSolucao();
    if (act === "usar-solucao" && atual.ex) {
      editor.trocar(atual.ex.solucao);
      toast("Solução copiada para o editor");
    }
    if (act === "copy-jsonl") {
      copiar(
        qjo.tokens(comp.src)
          .map((t) => JSON.stringify({ arquivo: atual.arquivo, linha: t.linha, coluna: t.coluna, token: t.token, lexema: t.lexema }))
          .join("\n"),
      );
    }
    if (act === "cpp-copiar") copiar(comp.cpp, "C++ copiado. Compile com g++ -std=c++17");
    if (act === "cpp-baixar") baixarCpp();
  }
  const r = alvo.closest<HTMLElement>("[data-resp]");
  if (r && sim && sim.esperando !== null) {
    let t = parseInt(($("sim-t") as HTMLInputElement | null)?.value ?? "", 10);
    t = isNaN(t) || t < 0 ? 0 : Math.min(t, sim.esperando);
    const resp = r.dataset.resp!;
    sim.respostas.push(resp === "TEMPO_ESGOTADO" ? resp : resp + " " + t);
    avancarSim();
  }
});
document.addEventListener("keydown", (e) => {
  // Dentro do editor o atalho é tratado pelo próprio CodeMirror.
  if ((e.metaKey || e.ctrlKey) && e.key === "Enter" && !editor.view.hasFocus) {
    e.preventDefault();
    atalhoRodar();
  }
});

abrir(RASCUNHO);
