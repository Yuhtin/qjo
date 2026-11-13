// CodeMirror 6 com a análise do qjo-wasm: realce por tokens, diagnósticos com correção,
// dica ao passar o mouse, autocompletar com trechos e ir à definição com Cmd/Ctrl+clique.

import { EditorState, StateEffect, StateField, type Extension, RangeSetBuilder } from "@codemirror/state";
import {
  Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate, keymap, lineNumbers,
  highlightActiveLine, highlightActiveLineGutter, drawSelection, hoverTooltip, highlightSpecialChars,
  rectangularSelection, crosshairCursor, dropCursor,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { indentUnit, bracketMatching, indentOnInput } from "@codemirror/language";
import { type Diagnostic, setDiagnostics, lintGutter, lintKeymap } from "@codemirror/lint";
import {
  autocompletion, type Completion, type CompletionContext, type CompletionResult, snippetCompletion,
  closeBrackets, closeBracketsKeymap, completionKeymap,
} from "@codemirror/autocomplete";
import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
import * as qjo from "./qjo";

// ---------- análise compartilhada ----------

export interface EstadoAnalise { texto: string; analise: qjo.Analise }

const definirAnalise = StateEffect.define<EstadoAnalise>();

export const campoAnalise = StateField.define<EstadoAnalise | null>({
  create: () => null,
  update(v, tr) {
    for (const e of tr.effects) if (e.is(definirAnalise)) return e.value;
    return v;
  },
});

const marcas = new Map<string, Decoration>();
function marca(cat: string): Decoration {
  let d = marcas.get(cat);
  if (!d) {
    d = Decoration.mark({ class: `tk-${cat}` });
    marcas.set(cat, d);
  }
  return d;
}

// Com erro de léxico ou sintaxe a semântica não roda e todo identificador vira "nome".
// Para as cores não piscarem enquanto se digita, use a categoria da última análise sem erros.
let categoriasConhecidas = new Map<string, qjo.Categoria>();

function decoracoes(a: qjo.Analise, texto: string): DecorationSet {
  const tam = texto.length;
  if (a.ok) {
    const m = new Map<string, qjo.Categoria>();
    for (const [cat, ini, fim] of a.tokens) if (cat !== "nome" && /^[A-Za-z]/.test(texto[ini])) m.set(texto.slice(ini, fim), cat);
    categoriasConhecidas = m;
  }
  const b = new RangeSetBuilder<Decoration>();
  for (let [cat, ini, fim] of a.tokens) {
    if (cat === "pontuacao" || fim <= ini || fim > tam) continue;
    if (!a.ok && cat === "nome") cat = categoriasConhecidas.get(texto.slice(ini, fim)) ?? cat;
    b.add(ini, fim, marca(cat));
  }
  return b.finish();
}

// Entre uma edição e a próxima análise, as marcas acompanham o texto (map).
const campoRealce = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    for (const e of tr.effects) {
      if (e.is(definirAnalise)) return decoracoes(e.value.analise, e.value.texto);
    }
    return tr.docChanged ? deco.map(tr.changes) : deco;
  },
  provide: (f) => EditorView.decorations.from(f),
});

function aplicarSugestao(view: EditorView, d: qjo.Diagnostico, texto: string) {
  let sug = d.sugestao;
  const atual = view.state.doc.toString();
  if (atual !== texto) {
    // O texto mudou desde a análise: pegue a sugestão equivalente na análise de agora.
    const novo = qjo.analisar(atual).diagnosticos
      .filter((x) => x.codigo === d.codigo && x.sugestao)
      .sort((a, b) => Math.abs(a.ini - d.ini) - Math.abs(b.ini - d.ini))[0];
    sug = novo?.sugestao ?? null;
  }
  if (!sug) return;
  view.dispatch({
    changes: sug.edicoes.map((e) => ({ from: e.ini, to: e.fim, insert: e.texto })),
    userEvent: "input.fix",
  });
  view.focus();
}

function paraDiagnosticos(e: EstadoAnalise, tam: number): Diagnostic[] {
  return e.analise.diagnosticos.map((d) => {
    const from = Math.min(d.ini, tam);
    const to = Math.min(Math.max(d.fim, d.ini), tam);
    return {
      from,
      to,
      severity: "error",
      source: `${d.codigo} · ${d.fase}`,
      message: d.mensagem,
      renderMessage: () => {
        const el = document.createElement("div");
        el.className = "diag";
        const cab = document.createElement("div");
        cab.className = "diag-cab";
        const cod = document.createElement("code");
        cod.className = "diag-codigo";
        cod.textContent = d.codigo;
        const fase = document.createElement("span");
        fase.className = "diag-fase";
        fase.textContent = `${d.fase} · linha ${d.linha}, coluna ${d.coluna}`;
        cab.append(cod, fase);
        const msg = document.createElement("p");
        msg.className = "diag-msg";
        msg.textContent = d.mensagem;
        el.append(cab, msg);
        return el;
      },
      actions: d.sugestao
        ? [{ name: d.sugestao.titulo, apply: (view: EditorView) => aplicarSugestao(view, d, e.texto) }]
        : [],
    };
  });
}

type Ouvinte = (e: EstadoAnalise) => void;

function plugAnalise(atraso: number, ouvir: Ouvinte) {
  return ViewPlugin.fromClass(
    class {
      timer = 0;
      constructor(readonly view: EditorView) {
        // Primeira análise logo depois da montagem (não dá para despachar no construtor).
        this.timer = window.setTimeout(() => this.rodar(), 0);
      }
      update(u: ViewUpdate) {
        if (!u.docChanged) return;
        clearTimeout(this.timer);
        this.timer = window.setTimeout(() => this.rodar(), atraso);
      }
      rodar() {
        const texto = this.view.state.doc.toString();
        const est: EstadoAnalise = { texto, analise: qjo.analisar(texto) };
        const diag = setDiagnostics(this.view.state, paraDiagnosticos(est, texto.length));
        const efeitos = Array.isArray(diag.effects) ? diag.effects : diag.effects ? [diag.effects] : [];
        this.view.dispatch({ effects: [definirAnalise.of(est), ...efeitos] });
        ouvir(est);
      }
      destroy() {
        clearTimeout(this.timer);
      }
    },
  );
}

// ---------- quebra de linha com recuo ----------

// Linhas quebradas continuam alinhadas ao recuo da linha, mais um nível.
const recuoDecos = new Map<number, Decoration>();
function recuoDeco(n: number): Decoration {
  let d = recuoDecos.get(n);
  if (!d) {
    // O recuo é limitado a 35% da largura (padding e text-indent em % usam a mesma base).
    d = Decoration.line({ attributes: { style: `--recuo: min(${n}ch, 35%)`, class: "cm-recuo" } });
    recuoDecos.set(n, d);
  }
  return d;
}

function decoRecuo(view: EditorView): DecorationSet {
  const b = new RangeSetBuilder<Decoration>();
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const linha = view.state.doc.lineAt(pos);
      const m = /^[ \t]*/.exec(linha.text)![0];
      let col = 0;
      for (const c of m) col += c === "\t" ? 4 : 1;
      if (linha.length > 0) b.add(linha.from, linha.from, recuoDeco(col + 4));
      pos = linha.to + 1;
    }
  }
  return b.finish();
}

const recuoQuebra = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = decoRecuo(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged || u.viewportChanged) this.decorations = decoRecuo(u.view);
    }
  },
  { decorations: (v) => v.decorations },
);

// ---------- dica ----------

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, txt?: string) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (txt !== undefined) e.textContent = txt;
  return e;
}

/** Realça um trecho de código fora do editor usando os tokens do analisador. */
export function realcarTrecho(codigo: string, destino: HTMLElement) {
  const toks = qjo.analisar(codigo).tokens;
  let pos = 0;
  for (const [cat, ini, fim] of toks) {
    if (ini > pos) destino.append(codigo.slice(pos, ini));
    destino.append(el("span", `tk-${cat}`, codigo.slice(ini, fim)));
    pos = fim;
  }
  if (pos < codigo.length) destino.append(codigo.slice(pos));
}

/** Markdown mínimo: blocos ```qjo, parágrafos, `código` e **negrito**. */
export function markdown(md: string): HTMLElement {
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
    return {
      pos: r.ini,
      end: r.fim,
      above: true,
      create: () => ({ dom: markdown(r.md) }),
    };
  },
  { hoverTime: 350 },
);

// ---------- autocompletar ----------

/** LSP `${1:x}`, `$1`, `$0` → CodeMirror `#{1:x}`, `#{1}`, `#{}`. */
export function converterTrecho(s: string): string {
  return s
    .replace(/\$\{(\d+):([^}]*)\}/g, (_, n, t) => (n === "0" ? `#{${t}}` : `#{${n}:${t}}`))
    .replace(/\$\{(\d+)\}/g, (_, n) => (n === "0" ? "#{}" : `#{${n}}`))
    .replace(/\$(\d+)/g, (_, n) => (n === "0" ? "#{}" : `#{${n}}`));
}

const tipoItem: Record<qjo.ItemCompletar["especie"], string> = {
  palavra: "keyword",
  tipo: "type",
  primitiva: "function",
  funcao: "method",
  variavel: "variable",
  constante: "constant",
  enum: "enum",
  trecho: "snippet",
};

const reforco: Partial<Record<qjo.ItemCompletar["especie"], number>> = {
  variavel: 3,
  constante: 2,
  primitiva: 1,
  trecho: -1,
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

export function irParaDefinicao(view: EditorView, pos: number): boolean {
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
    if (pos == null) return false;
    if (irParaDefinicao(view, pos)) {
      e.preventDefault();
      return true;
    }
    return false;
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
  aoAnalisar: Ouvinte;
  aoMudar: (texto: string) => void;
  aoRodar: () => void;
  aoFormatar: () => void;
}

export interface Editor {
  view: EditorView;
  /** Troca o texto inteiro (outro exemplo ou exercício), com histórico novo. */
  trocar(texto: string): void;
}

export function criarEditor(o: OpcoesEditor): Editor {
  const extensoes: Extension[] = [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightSpecialChars(),
    history(),
    drawSelection(),
    dropCursor(),
    EditorState.allowMultipleSelections.of(true),
    indentOnInput(),
    bracketMatching(),
    closeBrackets(),
    rectangularSelection(),
    crosshairCursor(),
    highlightActiveLine(),
    highlightSelectionMatches(),
    indentUnit.of("    "),
    EditorView.lineWrapping,
    recuoQuebra,
    EditorState.tabSize.of(4),
    EditorView.contentAttributes.of({ "aria-label": "Código Q?JO!", spellcheck: "false", autocapitalize: "off", autocorrect: "off" }),
    campoAnalise,
    campoRealce,
    plugAnalise(140, o.aoAnalisar),
    lintGutter(),
    dicaHover,
    autocompletion({ override: [fonteCompletar], icons: true, activateOnTypingDelay: 60 }),
    cliqueDefinicao,
    teclaModificadora,
    keymap.of([
      { key: "Mod-Enter", run: () => (o.aoRodar(), true), preventDefault: true },
      { key: "Shift-Alt-f", run: () => (o.aoFormatar(), true), preventDefault: true },
      { key: "F12", run: (v) => irParaDefinicao(v, v.state.selection.main.head) },
      ...closeBracketsKeymap,
      ...completionKeymap,
      ...defaultKeymap,
      ...searchKeymap,
      ...historyKeymap,
      ...lintKeymap,
      indentWithTab,
    ]),
    EditorView.updateListener.of((u) => {
      if (u.docChanged) o.aoMudar(u.state.doc.toString());
    }),
  ];
  const view = new EditorView({
    parent: o.pai,
    state: EditorState.create({ doc: o.texto, extensions: extensoes }),
  });
  return {
    view,
    trocar(texto: string) {
      // Um estado novo reinicia o histórico; o plugin de análise roda de novo ao ser recriado.
      view.setState(EditorState.create({ doc: texto, extensions: extensoes }));
      view.scrollDOM.scrollTop = 0;
    },
  };
}
