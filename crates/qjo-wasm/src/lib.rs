//! O núcleo da Q?JO! exposto ao navegador, sem wasm-bindgen: o JavaScript escreve o texto na
//! memória (`qjo_alloc`), chama uma função e lê o resultado em JSON (`qjo_saida_ptr`/`qjo_saida_len`).
//! Todas as posições na saída são deslocamentos UTF-16, como as strings do JavaScript.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;

use qjo_core::ide::{self, EspecieEsboco, EspecieItem};
use qjo_core::interp::{Roteiro, executar as exec};
use qjo_core::sema::Especie;
use qjo_core::{Analise, Tok, analisar, cpp, fmt, ler_respostas};

thread_local! {
    static SAIDA: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Byte -> UTF-16 e o inverso.
struct Pos16 {
    b2u: Vec<u32>,
}

impl Pos16 {
    fn new(s: &str) -> Self {
        let mut b2u = vec![0u32; s.len() + 1];
        let mut u = 0u32;
        for (i, c) in s.char_indices() {
            for k in 0..c.len_utf8() {
                b2u[i + k] = u;
            }
            u += c.len_utf16() as u32;
        }
        b2u[s.len()] = u;
        Pos16 { b2u }
    }
    fn u(&self, b: usize) -> u32 {
        self.b2u[b.min(self.b2u.len() - 1)]
    }
    fn b(&self, u: u32) -> u32 {
        self.b2u.partition_point(|&x| x < u) as u32
    }
}

fn js(o: &mut String, s: &str) {
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 32 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
}

fn categoria(a: &Analise, t: &qjo_core::Token, esp: &HashMap<u32, Especie>) -> &'static str {
    match t.tipo {
        Tok::Se
        | Tok::SenaoSe
        | Tok::Senao
        | Tok::Para
        | Tok::Enquanto
        | Tok::Faca
        | Tok::Retorna => "controle",
        Tok::Rotina | Tok::Etapa | Tok::Funcao => "bloco",
        Tok::Tipo => "tipo",
        Tok::Logico | Tok::Nulo => "constante",
        Tok::Inteiro => "numero",
        Tok::Duracao => "duracao",
        Tok::Data => "data",
        Tok::Texto => "texto",
        Tok::Coment => "comentario",
        Tok::Id => match esp.get(&t.ini) {
            Some(Especie::Primitiva) => "primitiva",
            Some(Especie::ValorEnum) => "enum",
            Some(Especie::Funcao) => "funcao",
            Some(Especie::Recurso | Especie::Horario) => "recurso",
            Some(Especie::Enum) => "tipo",
            _ => {
                let _ = a;
                "nome"
            }
        },
        Tok::AbrePar
        | Tok::FechaPar
        | Tok::AbreChave
        | Tok::FechaChave
        | Tok::Sep
        | Tok::PontoVirgula => "pontuacao",
        _ => "operador",
    }
}

fn esboco_json(o: &mut String, n: &[ide::NoEsboco], p: &Pos16) {
    o.push('[');
    for (i, x) in n.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        o.push_str("{\"nome\":");
        js(o, &x.nome);
        let esp = match x.especie {
            EspecieEsboco::Rotina => "rotina",
            EspecieEsboco::Etapa => "etapa",
            EspecieEsboco::Funcao => "funcao",
            EspecieEsboco::Enum => "enum",
            EspecieEsboco::Recurso => "recurso",
            EspecieEsboco::Horario => "horario",
        };
        let _ = write!(
            o,
            ",\"especie\":\"{esp}\",\"ini\":{},\"fim\":{},\"filhos\":",
            p.u(x.selecao.ini as usize),
            p.u(x.selecao.fim as usize)
        );
        esboco_json(o, &x.filhos, p);
        o.push('}');
    }
    o.push(']');
}

/// `{ok, diagnosticos, tokens, esboco}`.
pub fn analisar_json(src: &str) -> String {
    let a = analisar(src);
    let p = Pos16::new(src);
    let mut o = String::with_capacity(src.len() * 4);
    let _ = write!(o, "{{\"ok\":{},\"diagnosticos\":[", a.ok());
    for (i, d) in a.diagnosticos.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(
            o,
            "{{\"codigo\":\"{}\",\"fase\":\"{}\",\"linha\":{},\"coluna\":{},\"ini\":{},\"fim\":{},\"mensagem\":",
            d.codigo,
            d.fase.nome(),
            d.linha,
            d.coluna,
            p.u(d.ini),
            p.u(d.fim)
        );
        js(&mut o, &d.mensagem);
        o.push_str(",\"sugestao\":");
        match &d.sugestao {
            None => o.push_str("null"),
            Some(s) => {
                o.push_str("{\"titulo\":");
                js(&mut o, &s.titulo);
                o.push_str(",\"edicoes\":[");
                for (k, e) in s.edicoes.iter().enumerate() {
                    if k > 0 {
                        o.push(',');
                    }
                    let _ = write!(
                        o,
                        "{{\"ini\":{},\"fim\":{},\"texto\":",
                        p.u(e.ini),
                        p.u(e.fim)
                    );
                    js(&mut o, &e.texto);
                    o.push('}');
                }
                o.push_str("]}");
            }
        }
        o.push('}');
    }
    let esp: HashMap<u32, Especie> = a
        .info
        .usos
        .iter()
        .map(|(sp, s)| (sp.ini, a.info.simbolos[*s].especie))
        .collect();
    o.push_str("],\"tokens\":[");
    for (i, t) in a.tokens.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(
            o,
            "[\"{}\",{},{}]",
            categoria(&a, t, &esp),
            p.u(t.ini as usize),
            p.u(t.fim as usize)
        );
    }
    o.push_str("],\"esboco\":");
    esboco_json(&mut o, &ide::esboco(&a.programa), &p);
    o.push('}');
    o
}

/// Executa repetindo do começo com as respostas dadas. Quando elas acabam, pausa e informa o
/// limite da espera (`pausada`): o chamador pergunta à pessoa e chama de novo com uma a mais.
pub fn executar_json(src: &str, respostas: &str) -> String {
    let a = analisar(src);
    if !a.ok() {
        return "{\"ok\":false}".into();
    }
    let Ok(r) = ler_respostas(respostas) else {
        return "{\"ok\":false,\"erro\":\"respostas inválidas\"}".into();
    };
    let mut rot = Roteiro::new(r);
    rot.pausar_no_fim = true;
    let e = exec(&a.programa, &a.info, &mut rot);
    let mut o = String::from("{\"ok\":true,\"registro\":[");
    for (i, l) in rot.saida.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        js(&mut o, l);
    }
    o.push_str("],\"rotinas\":[");
    for (i, (n, st)) in e.rotinas.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        o.push_str("{\"nome\":");
        js(&mut o, n);
        o.push_str(",\"estado\":");
        js(&mut o, &st.texto());
        o.push('}');
    }
    let _ = write!(
        o,
        "],\"pausada\":{}}}",
        e.pausada.map(|l| l.to_string()).unwrap_or("null".into())
    );
    o
}

pub fn completar_json(src: &str, off16: u32) -> String {
    let a = analisar(src);
    let p = Pos16::new(src);
    let mut o = String::from("[");
    for (i, it) in ide::completar(&a, p.b(off16)).iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let esp = match it.especie {
            EspecieItem::Palavra => "palavra",
            EspecieItem::Tipo => "tipo",
            EspecieItem::Primitiva => "primitiva",
            EspecieItem::Funcao => "funcao",
            EspecieItem::Variavel => "variavel",
            EspecieItem::Constante => "constante",
            EspecieItem::ValorEnum => "enum",
            EspecieItem::Enum => "tipo",
            EspecieItem::Trecho => "trecho",
        };
        o.push_str("{\"rotulo\":");
        js(&mut o, &it.rotulo);
        let _ = write!(o, ",\"especie\":\"{esp}\",\"detalhe\":");
        js(&mut o, &it.detalhe);
        o.push_str(",\"snippet\":");
        match &it.snippet {
            Some(s) => js(&mut o, s),
            None => o.push_str("null"),
        }
        o.push('}');
    }
    o.push(']');
    o
}

pub fn dica_json(src: &str, off16: u32) -> String {
    let a = analisar(src);
    let p = Pos16::new(src);
    match ide::dica(&a, src, p.b(off16)) {
        None => "null".into(),
        Some((sp, md)) => {
            let mut o = format!(
                "{{\"ini\":{},\"fim\":{},\"md\":",
                p.u(sp.ini as usize),
                p.u(sp.fim as usize)
            );
            js(&mut o, &md);
            o.push('}');
            o
        }
    }
}

pub fn definicao_json(src: &str, off16: u32) -> String {
    let a = analisar(src);
    let p = Pos16::new(src);
    match ide::definicao(&a, p.b(off16)) {
        None => "null".into(),
        Some(sp) => format!(
            "{{\"ini\":{},\"fim\":{}}}",
            p.u(sp.ini as usize),
            p.u(sp.fim as usize)
        ),
    }
}

/// Saída do léxico no formato de `qjo tokens --json`: `[{linha, coluna, token, lexema}]`.
pub fn tokens_json(src: &str) -> String {
    let lx = qjo_core::lex(src);
    let mut o = String::with_capacity(src.len() * 6);
    o.push('[');
    for (i, t) in lx.tokens.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(
            o,
            "{{\"linha\":{},\"coluna\":{},\"token\":\"{}\",\"lexema\":",
            t.linha,
            t.coluna,
            t.tipo.nome()
        );
        js(&mut o, t.lexema(src));
        o.push('}');
    }
    o.push(']');
    o
}

pub fn cpp_texto(src: &str) -> String {
    let a = analisar(src);
    if a.ok() {
        cpp::gerar(&a.programa, &a.info, "rotina.qjo")
    } else {
        String::new()
    }
}

// ---------- interface C para o JavaScript ----------

fn texto(ptr: *const u8, len: usize) -> &'static str {
    // SAFETY: o JavaScript escreveu `len` bytes UTF-8 válidos (TextEncoder) em memória vinda de qjo_alloc.
    unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len)) }
}

fn guardar(s: String) {
    SAIDA.with(|o| *o.borrow_mut() = s);
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` e `len` vieram de `qjo_alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qjo_free(ptr: *mut u8, len: usize) {
    // SAFETY: garantido pelo chamador.
    unsafe { drop(Vec::from_raw_parts(ptr, 0, len.max(1))) }
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_saida_ptr() -> *const u8 {
    SAIDA.with(|o| o.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_saida_len() -> usize {
    SAIDA.with(|o| o.borrow().len())
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_analisar(p: *const u8, n: usize) {
    guardar(analisar_json(texto(p, n)));
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_executar(p: *const u8, n: usize, rp: *const u8, rn: usize) {
    guardar(executar_json(texto(p, n), texto(rp, rn)));
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_completar(p: *const u8, n: usize, off16: u32) {
    guardar(completar_json(texto(p, n), off16));
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_dica(p: *const u8, n: usize, off16: u32) {
    guardar(dica_json(texto(p, n), off16));
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_definicao(p: *const u8, n: usize, off16: u32) {
    guardar(definicao_json(texto(p, n), off16));
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_tokens(p: *const u8, n: usize) {
    guardar(tokens_json(texto(p, n)));
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_cpp(p: *const u8, n: usize) {
    guardar(cpp_texto(texto(p, n)));
}

#[unsafe(no_mangle)]
pub extern "C" fn qjo_formatar(p: *const u8, n: usize) {
    guardar(fmt::formatar(texto(p, n)).unwrap_or_default());
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn utf16_e_pausa() {
        let src = "# ação ✓\ndata h = data\"08:00:00\";\nrotina(\"R\", h) {\n    resultado r = AGUARDAR_CONFIRMACAO(30s);\n    ESPERAR(1s);\n}\n";
        let j = analisar_json(src);
        assert!(j.starts_with("{\"ok\":true"));
        // "data" começa no byte 13 ("ação ✓" tem 3 bytes a mais em UTF-8 que em UTF-16).
        assert!(j.contains("[\"tipo\",9,13]"), "{j}");
        let t = tokens_json(src);
        assert!(
            t.starts_with("[{\"linha\":1,\"coluna\":1,\"token\":\"COMENT\""),
            "{t}"
        );
        let e = executar_json(src, "");
        assert!(e.contains("\"pausada\":30"), "{e}");
        let e = executar_json(src, "AJUDA 2");
        assert!(
            e.contains("AGUARDAR_CONFIRMACAO 30s -> AJUDA (2s)") && e.contains("\"pausada\":null"),
            "{e}"
        );
    }
}
