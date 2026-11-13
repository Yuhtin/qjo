// CodeMirror 6 com a análise do qjo-wasm, vestido como o editor da QueijoCode: mesmas classes
// de cor, erro sublinhado em onda, número da linha em vermelho. Por baixo continuam a dica ao
// passar o mouse, o autocompletar com trechos e ir à definição com Cmd/Ctrl+clique.

import { EditorState, RangeSet, RangeSetBuilder, StateField, type Extension } from "@codemirror/state";
import {
  Decoration, type DecorationSet, EditorView, GutterMarker, ViewPlugin, WidgetType, keymap, lineNumbers,
  gutterLineClass, drawSelection, hoverTooltip, highlightSpecialChars, dropCursor,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { indentUnit, indentService } from "@codemirror/language";
import {
  autocompletion, type Completion, type CompletionContext, type CompletionResult, snippetCompletion,
  completionKeymap,
} from "@codemirror/autocomplete";
import { searchKeymap } from "@codemirror/search";
import * as qjo from "./qjo";

// ---------- análise ----------

export interface EstadoAnalise { texto: string; analise: qjo.Analise }

const analisar = (texto: string): EstadoAnalise => ({ texto, analise: qjo.analisar(texto) });

/** A análise acompanha o documento a cada mudança (o núcleo leva menos de 1 ms). */
export const campoAnalise = StateField.define<EstadoAnalise>({
  create: (st) => analisar(st.doc.toString()),
  update: (v, tr) => (tr.docChanged ? analisar(tr.newDoc.toString()) : v),
});

/** Categoria do qjo-wasm -> classe de cor da QueijoCode. */
const CLASSE: Partial<Record<qjo.Categoria, string>> = {
  controle: "kw", bloco: "kw", tipo: "ty", texto: "str", comentario: "com",
  numero: "lit", duracao: "lit", data: "lit", constante: "lit", primitiva: "prim", enum: "enumv",
};

export function classeDe(cat: qjo.Categoria): string | undefined {
  return CLASSE[cat];
}

const marcas = new Map<string, Decoration>();
function marca(cls: string): Decoration {
  let d = marcas.get(cls);
  if (!d) marcas.set(cls, (d = Decoration.mark({ class: cls })));
  return d;
}

// Com erro de léxico ou sintaxe a semântica não roda e todo identificador vira "nome".
// Para as cores não piscarem enquanto se digita, use a categoria da última análise sem erros.
let conhecidas = new Map<string, qjo.Categoria>();

function realce(e: EstadoAnalise): DecorationSet {
  const { analise: a, texto } = e;
  if (a.ok) {
    const m = new Map<string, qjo.Categoria>();
    for (const [cat, ini, fim] of a.tokens) if (cat !== "nome" && /^[A-Za-z]/.test(texto[ini])) m.set(texto.slice(ini, fim), cat);
    conhecidas = m;
  }
  const b = new RangeSetBuilder<Decoration>();
  for (let [cat, ini, fim] of a.tokens) {
    if (fim <= ini || fim > texto.length) continue;
    if (!a.ok && cat === "nome") cat = conhecidas.get(texto.slice(ini, fim)) ?? cat;
    const cls = CLASSE[cat];
    if (cls) b.add(ini, fim, marca(cls));
  }
  return b.finish();
}

class PontoDeErro extends WidgetType {
  toDOM() {
    const s = document.createElement("span");
    s.className = "errpt";
    return s;
  }
  eq() {
    return true;
  }
}
const pontoErro = Decoration.widget({ widget: new PontoDeErro(), side: 1 });
const trechoErro = Decoration.mark({ class: "err" });

/** Erro com extensão: sublinhado em onda. Erro sem extensão (falta algo): um marcador no ponto. */
function erros(e: EstadoAnalise): DecorationSet {
  const { texto } = e;
  const n = texto.length;
  const faixas: { de: number; ate: number; d: Decoration }[] = [];
  for (const d of e.analise.diagnosticos) {
    const a = Math.min(d.ini, n);
    const b = Math.min(Math.max(d.fim, d.ini), n);
    if (b === a || a >= n || texto[a] === "\n") faixas.push({ de: a, ate: a, d: pontoErro });
    else faixas.push({ de: a, ate: b, d: trechoErro });
  }
  faixas.sort((x, y) => x.de - y.de || x.ate - y.ate);
  return Decoration.set(faixas.map((f) => f.d.range(f.de, f.ate)), true);
}

class LinhaComErro extends GutterMarker {
  elementClass = "e";
}
const linhaErro = new LinhaComErro();

function linhasComErro(st: EditorState): RangeSet<GutterMarker> {
  const { analise } = st.field(campoAnalise);
  const vistas = new Set<number>();
  const pos: number[] = [];
  for (const d of analise.diagnosticos) {
    const l = st.doc.lineAt(Math.min(d.ini, st.doc.length));
    if (!vistas.has(l.number)) {
      vistas.add(l.number);
      pos.push(l.from);
    }
  }
  pos.sort((a, b) => a - b);
  return RangeSet.of(pos.map((p) => linhaErro.range(p)));
}

// ---------- recuo (o mesmo da QueijoCode) ----------

const recuoDe = (s: string) => /^[ \t]*/.exec(s)![0].replace(/\t/g, "    ").length;

/** Enter depois de `{` entra um nível; fora isso mantém o recuo da linha. */
const recuo = indentService.of((cx, pos) => {
  // No Enter, lineAt(pos, -1) é o texto da linha até o cursor.
  const antes = cx.lineAt(pos, -1).text;
  return recuoDe(antes) + (/\{\s*$/.test(antes) ? 4 : 0);
});

/** `}` digitado numa linha só de espaços volta um nível. */
const fechaChave = EditorView.inputHandler.of((view, from, to, texto) => {
  if (texto !== "}") return false;
  const linha = view.state.doc.lineAt(from);
  const pre = linha.text.slice(0, from - linha.from);
  if (!/^ {4,}$/.test(pre)) return false;
  view.dispatch({
    changes: { from: linha.from, to, insert: pre.slice(4) + "}" },
    selection: { anchor: linha.from + pre.length - 3 },
    userEvent: "input.type",
  });
  return true;
});

// ---------- dica ----------

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, txt?: string) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (txt !== undefined) e.textContent = txt;
  return e;
}

/** Realça um trecho de código fora do editor com as mesmas classes. */
export function realcarTrecho(codigo: string, destino: HTMLElement) {
  const toks = qjo.analisar(codigo).tokens;
  let pos = 0;
  for (const [cat, ini, fim] of toks) {
    if (ini > pos) destino.append(codigo.slice(pos, ini));
    const cls = CLASSE[cat];
    destino.append(cls ? el("span", cls, codigo.slice(ini, fim)) : codigo.slice(ini, fim));
    pos = fim;
  }
  if (pos < codigo.length) destino.append(codigo.slice(pos));
}

/** Markdown mínimo: blocos ```qjo, parágrafos, `código` e **negrito**. */
function markdown(md: string): HTMLElement {
  const raiz = el("div", "md");
  const partes = md.split(/```(\w*)\n([\s\S]*?)```/g);
  for (let i = 0; i < partes.length; i += 3) {
    for (const par of partes[i].split(/\n{2,}/)) {
      const t = par.trim();
      if (!t) continue;
      const p = el("p");
      for (const [k, pedaco] of t.split(/(`[^`]+`|\*\*[^*]+\*\*)/g).entries()) {
        if (k % 2 === 0) p.append(pedaco);
        else if (pedaco.startsWith("`")) p.append(el("code", undefined, pedaco.slice(1, -1)));
        else p.append(el("strong", undefined, pedaco.slice(2, -2)));
      }
      raiz.append(p);
    }
    if (i + 2 < partes.length) {
      const pre = el("pre");
      const codigo = partes[i + 2].replace(/\n$/, "");
      if (partes[i + 1] === "qjo") realcarTrecho(codigo, pre);
      else pre.textContent = codigo;
      raiz.append(pre);
    }
  }
  return raiz;
}

const dicaHover = hoverTooltip(
  (view, pos) => {
    const r = qjo.dica(view.state.doc.toString(), pos);
    if (!r) return null;
    return { pos: r.ini, end: r.fim, above: true, create: () => ({ dom: markdown(r.md) }) };
  },
  { hoverTime: 350 },
);

// ---------- autocompletar ----------

/** LSP `${1:x}`, `$1`, `$0` -> CodeMirror `#{1:x}`, `#{1}`, `#{}`. */
function converterTrecho(s: string): string {
  return s
    .replace(/\$\{(\d+):([^}]*)\}/g, (_, n, t) => (n === "0" ? `#{${t}}` : `#{${n}:${t}}`))
    .replace(/\$\{(\d+)\}/g, (_, n) => (n === "0" ? "#{}" : `#{${n}}`))
    .replace(/\$(\d+)/g, (_, n) => (n === "0" ? "#{}" : `#{${n}}`));
}

const tipoItem: Record<qjo.ItemCompletar["especie"], string> = {
  palavra: "keyword", tipo: "type", primitiva: "function", funcao: "method",
  variavel: "variable", constante: "constant", enum: "enum", trecho: "snippet",
};

const reforco: Partial<Record<qjo.ItemCompletar["especie"], number>> = {
  variavel: 3, constante: 2, primitiva: 1, trecho: -1,
};

function fonteCompletar(ctx: CompletionContext): CompletionResult | null {
  const palavra = ctx.matchBefore(/[A-Za-z_][A-Za-z0-9_]*[?!]?/);
  if (!palavra && !ctx.explicit) return null;
  const itens = qjo.completar(ctx.state.doc.toString(), ctx.pos);
  const vistos = new Set<string>();
  const opcoes: Completion[] = [];
  for (const it of itens) {
    const chave = `${it.rotulo}|${it.snippet ?? ""}`;
    if (vistos.has(chave)) continue;
    vistos.add(chave);
    const base = { label: it.rotulo, detail: it.detalhe, type: tipoItem[it.especie], boost: reforco[it.especie] ?? 0 };
    opcoes.push(it.snippet ? snippetCompletion(converterTrecho(it.snippet), base) : base);
  }
  return { from: palavra ? palavra.from : ctx.pos, options: opcoes, validFor: /^[A-Za-z0-9_]*[?!]?$/ };
}

// ---------- definição ----------

function irParaDefinicao(view: EditorView, pos: number): boolean {
  const d = qjo.definicao(view.state.doc.toString(), pos);
  if (!d) return false;
  view.dispatch({
    selection: { anchor: d.ini, head: d.fim },
    effects: EditorView.scrollIntoView(d.ini, { y: "center" }),
    userEvent: "select.definition",
  });
  view.focus();
  return true;
}

const cliqueDefinicao = EditorView.domEventHandlers({
  mousedown(e, view) {
    if (!(e.metaKey || e.ctrlKey) || e.button !== 0) return false;
    const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
    if (pos == null || !irParaDefinicao(view, pos)) return false;
    e.preventDefault();
    return true;
  },
});

// Com Cmd/Ctrl pressionado, o editor ganha uma classe que sublinha nomes clicáveis.
const teclaModificadora = ViewPlugin.fromClass(
  class {
    constructor(readonly view: EditorView) {
      window.addEventListener("keydown", this.tecla);
      window.addEventListener("keyup", this.tecla);
      window.addEventListener("blur", this.solta);
    }
    tecla = (e: KeyboardEvent) => this.view.dom.classList.toggle("mod-ativo", e.metaKey || e.ctrlKey);
    solta = () => this.view.dom.classList.remove("mod-ativo");
    destroy() {
      window.removeEventListener("keydown", this.tecla);
      window.removeEventListener("keyup", this.tecla);
      window.removeEventListener("blur", this.solta);
    }
  },
);

// ---------- formatar ----------

/** Substitui só o trecho que mudou, para preservar cursor e histórico. */
export function formatarDoc(view: EditorView): boolean {
  const atual = view.state.doc.toString();
  const novo = qjo.formatar(atual);
  if (!novo) return false;
  if (novo === atual) return true;
  let ini = 0;
  while (ini < atual.length && ini < novo.length && atual[ini] === novo[ini]) ini++;
  let fa = atual.length;
  let fn = novo.length;
  while (fa > ini && fn > ini && atual[fa - 1] === novo[fn - 1]) {
    fa--;
    fn--;
  }
  view.dispatch({ changes: { from: ini, to: fa, insert: novo.slice(ini, fn) }, userEvent: "format" });
  return true;
}

// ---------- montagem ----------

export interface OpcoesEditor {
  pai: HTMLElement;
  texto: string;
  /** Chamado com a análise nova sempre que o texto muda (e ao trocar de arquivo). */
  aoMudar: (e: EstadoAnalise) => void;
  aoRodar: () => void;
  aoFormatar: () => void;
}

export interface Editor {
  view: EditorView;
  texto(): string;
  analise(): EstadoAnalise;
  /** Troca o texto inteiro (outro exercício), com histórico novo. */
  trocar(texto: string): void;
  /** Aplica edições de uma correção sugerida e põe o cursor no fim da última. */
  editar(edicoes: qjo.Edicao[]): void;
  irPara(ini: number, fim: number): void;
}

export function criarEditor(o: OpcoesEditor): Editor {
  const extensoes: Extension[] = [
    lineNumbers(),
    gutterLineClass.compute([campoAnalise], linhasComErro),
    highlightSpecialChars(),
    history(),
    drawSelection(),
    dropCursor(),
    indentUnit.of("    "),
    EditorState.tabSize.of(4),
    recuo,
    fechaChave,
    EditorView.contentAttributes.of({ "aria-label": "Código Q?JO!", spellcheck: "false", autocapitalize: "off", autocorrect: "off" }),
    campoAnalise,
    EditorView.decorations.compute([campoAnalise], (st) => realce(st.field(campoAnalise))),
    EditorView.decorations.compute([campoAnalise], (st) => erros(st.field(campoAnalise))),
    dicaHover,
    autocompletion({ override: [fonteCompletar], icons: false, activateOnTypingDelay: 60 }),
    cliqueDefinicao,
    teclaModificadora,
    keymap.of([
      { key: "Mod-Enter", run: () => (o.aoRodar(), true), preventDefault: true },
      { key: "Shift-Alt-f", run: () => (o.aoFormatar(), true), preventDefault: true },
      { key: "F12", run: (v) => irParaDefinicao(v, v.state.selection.main.head) },
      ...completionKeymap,
      ...defaultKeymap,
      ...searchKeymap,
      ...historyKeymap,
      indentWithTab,
    ]),
    EditorView.updateListener.of((u) => {
      if (u.docChanged) o.aoMudar(u.state.field(campoAnalise));
    }),
  ];
  const view = new EditorView({
    parent: o.pai,
    state: EditorState.create({ doc: o.texto, extensions: extensoes }),
  });
  return {
    view,
    texto: () => view.state.doc.toString(),
    analise: () => view.state.field(campoAnalise),
    trocar(texto: string) {
      view.setState(EditorState.create({ doc: texto, extensions: extensoes }));
      view.scrollDOM.scrollTop = 0;
      o.aoMudar(view.state.field(campoAnalise));
    },
    editar(edicoes) {
      const ord = [...edicoes].sort((a, b) => a.ini - b.ini);
      const changes = view.state.changes(ord.map((e) => ({ from: e.ini, to: e.fim, insert: e.texto })));
      const ult = ord[ord.length - 1];
      const fim = ult ? changes.mapPos(ult.ini) + ult.texto.length : 0;
      view.dispatch({ changes, selection: { anchor: Math.min(fim, changes.newLength) }, userEvent: "input.fix", scrollIntoView: true });
      view.focus();
    },
    irPara(ini, fim) {
      view.dispatch({
        selection: { anchor: ini, head: Math.max(ini, fim) },
        effects: EditorView.scrollIntoView(ini, { y: "center" }),
      });
      view.focus();
    },
  };
}
