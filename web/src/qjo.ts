// Ponte com crates/qjo-wasm: sem wasm-bindgen. O texto vai para a memória do módulo
// (qjo_alloc), a função grava a saída num buffer interno e lemos com qjo_saida_ptr/len.
// Todas as posições são deslocamentos UTF-16, ou seja, índices de string do JavaScript.

export type Categoria =
  | "controle" | "bloco" | "tipo" | "constante" | "numero" | "duracao" | "data" | "texto"
  | "comentario" | "primitiva" | "enum" | "funcao" | "recurso" | "nome" | "pontuacao" | "operador";

export interface Edicao { ini: number; fim: number; texto: string }
export interface Sugestao { titulo: string; edicoes: Edicao[] }
export interface Diagnostico {
  codigo: string;
  fase: string;
  linha: number;
  coluna: number;
  ini: number;
  fim: number;
  mensagem: string;
  sugestao: Sugestao | null;
}
export type EspecieEsboco = "rotina" | "etapa" | "funcao" | "enum" | "recurso" | "horario";
export interface NoEsboco { nome: string; especie: EspecieEsboco; ini: number; fim: number; filhos: NoEsboco[] }
export interface Analise {
  ok: boolean;
  diagnosticos: Diagnostico[];
  tokens: [Categoria, number, number][];
  esboco: NoEsboco[];
}
export interface RotinaFinal { nome: string; estado: string }
export type Execucao =
  | { ok: true; registro: string[]; rotinas: RotinaFinal[]; pausada: number | null }
  | { ok: false; erro?: string };
export interface ItemCompletar {
  rotulo: string;
  especie: "palavra" | "tipo" | "primitiva" | "funcao" | "variavel" | "constante" | "enum" | "trecho";
  detalhe: string;
  snippet: string | null;
}
export interface TokenLexico { linha: number; coluna: number; token: string; lexema: string }
export interface Dica { ini: number; fim: number; md: string }
export interface Faixa { ini: number; fim: number }

interface Exports {
  memory: WebAssembly.Memory;
  qjo_alloc(len: number): number;
  qjo_free(ptr: number, len: number): void;
  qjo_saida_ptr(): number;
  qjo_saida_len(): number;
  qjo_analisar(p: number, n: number): void;
  qjo_executar(p: number, n: number, rp: number, rn: number): void;
  qjo_completar(p: number, n: number, off16: number): void;
  qjo_dica(p: number, n: number, off16: number): void;
  qjo_definicao(p: number, n: number, off16: number): void;
  qjo_cpp(p: number, n: number): void;
  qjo_tokens(p: number, n: number): void;
  qjo_formatar(p: number, n: number): void;
}

const enc = new TextEncoder();
const dec = new TextDecoder();
let x: Exports | null = null;

export async function carregar(url: string): Promise<void> {
  const resp = await fetch(url);
  if (!resp.ok) throw new Error(`não foi possível baixar ${url} (${resp.status})`);
  let inst: WebAssembly.Instance;
  try {
    inst = (await WebAssembly.instantiateStreaming(resp.clone(), {})).instance;
  } catch {
    // Servidor sem Content-Type application/wasm.
    inst = (await WebAssembly.instantiate(await resp.arrayBuffer(), {})).instance;
  }
  x = inst.exports as unknown as Exports;
}

function mod(): Exports {
  if (!x) throw new Error("qjo.wasm ainda não foi carregado");
  return x;
}

function escrever(s: string): [number, number] {
  const m = mod();
  const b = enc.encode(s);
  const p = m.qjo_alloc(b.length);
  // A memória pode ter crescido no alloc: pegue o buffer depois dele.
  new Uint8Array(m.memory.buffer, p, b.length).set(b);
  return [p, b.length];
}

function lerSaida(): string {
  const m = mod();
  return dec.decode(new Uint8Array(m.memory.buffer, m.qjo_saida_ptr(), m.qjo_saida_len()));
}

function chamar(fn: (p: number, n: number) => void, src: string): string {
  const [p, n] = escrever(src);
  try {
    fn(p, n);
    return lerSaida();
  } finally {
    mod().qjo_free(p, n);
  }
}

// A análise é chamada por várias partes da interface com o mesmo texto: guarde a última.
let ultimo: { src: string; a: Analise } | null = null;

export function analisar(src: string): Analise {
  if (ultimo && ultimo.src === src) return ultimo.a;
  const a = JSON.parse(chamar((p, n) => mod().qjo_analisar(p, n), src)) as Analise;
  ultimo = { src, a };
  return a;
}

export function executar(src: string, respostas: string): Execucao {
  const m = mod();
  const [p, n] = escrever(src);
  const [rp, rn] = escrever(respostas);
  try {
    m.qjo_executar(p, n, rp, rn);
    return JSON.parse(lerSaida()) as Execucao;
  } finally {
    m.qjo_free(p, n);
    m.qjo_free(rp, rn);
  }
}

export function completar(src: string, off: number): ItemCompletar[] {
  return JSON.parse(chamar((p, n) => mod().qjo_completar(p, n, off), src)) as ItemCompletar[];
}

export function dica(src: string, off: number): Dica | null {
  return JSON.parse(chamar((p, n) => mod().qjo_dica(p, n, off), src)) as Dica | null;
}

export function definicao(src: string, off: number): Faixa | null {
  return JSON.parse(chamar((p, n) => mod().qjo_definicao(p, n, off), src)) as Faixa | null;
}

/** Saída do léxico, no mesmo formato de `qjo tokens --json`. */
export function tokens(src: string): TokenLexico[] {
  return JSON.parse(chamar((p, n) => mod().qjo_tokens(p, n), src)) as TokenLexico[];
}

/** Texto C++17, ou "" se o programa tem erros. */
export function cpp(src: string): string {
  return chamar((p, n) => mod().qjo_cpp(p, n), src);
}

/** Texto formatado, ou "" se o programa tem erros. */
export function formatar(src: string): string {
  return chamar((p, n) => mod().qjo_formatar(p, n), src);
}
