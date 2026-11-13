//! Gerador de C++17 (spec/05-cpp.md). O programa precisa estar sem erros.

use std::fmt::Write;

use crate::ast::*;
use crate::interp::{DIA_BASE, LIMITE_PASSOS, LIMITE_ROTINA};
use crate::sema::{Info, ValorGlobal};

const RUNTIME: &str = r#"#include <algorithm>
#include <cstdint>
#include <iostream>
#include <sstream>
#include <string>
#include <vector>

namespace qjo {

struct Erro { const char* motivo; };
struct Cancelar {};
struct Recurso { const char* nome; const char* descricao; };
struct Data { bool completa; int64_t seg; };

const char* const RESULTADO[] = {"CONFIRMADO", "AJUDA", "TEMPO_ESGOTADO"};
const char* const INTENSIDADE[] = {"BAIXA", "MEDIA", "ALTA"};
const char* const EVENTO[] = {"ETAPA_PULADA", "AJUDA_REPETIDA", "CONFIRMACAO_RAPIDA", "ROTINA_INCOMPLETA"};

int64_t inicio = 0, agora = 0, ultima = 0;
uint64_t passos = 0;
std::vector<const Recurso*> abertos;

inline int32_t num(int64_t v) {
    if (v < INT32_MIN || v > INT32_MAX) throw Erro{"estouro de num"};
    return static_cast<int32_t>(v);
}
inline int32_t neg(int32_t a) { return num(-static_cast<int64_t>(a)); }
inline int32_t div(int32_t a, int32_t b) {
    if (b == 0) throw Erro{"divisão por zero"};
    return num(static_cast<int64_t>(a) / b);
}
inline int32_t resto(int32_t a, int32_t b) {
    if (b == 0) throw Erro{"divisão por zero"};
    return b == -1 ? 0 : a % b;
}
inline int64_t dur(int64_t v) {
    if (v < 0) throw Erro{"duração negativa"};
    if (v > INT32_MAX) throw Erro{"estouro de duração"};
    return v;
}
inline int64_t dur_div(int64_t a, int32_t b) {
    if (b == 0) throw Erro{"divisão por zero"};
    return dur(a / b);
}
inline int64_t hora_do_dia(Data d) { return d.completa ? ((d.seg % 86400) + 86400) % 86400 : d.seg; }
inline int cmp(Data a, Data b) {
    int64_t x = a.seg, y = b.seg;
    if (!(a.completa && b.completa)) { x = hora_do_dia(a); y = hora_do_dia(b); }
    return (x > y) - (x < y);
}

inline void avancar(int64_t s) {
    agora += s;
    if (agora - inicio > LIMITE_ROTINA) throw Erro{"a rotina excedeu o limite de duas horas"};
}
inline void passo() {
    if (++passos > LIMITE_PASSOS) throw Erro{"possível laço sem fim (limite de passos)"};
}
inline void fechar(const char* motivo) {
    std::vector<const Recurso*> a;
    a.swap(abertos);
    for (const Recurso* o : a) std::cout << "PARAR_INDICACAO " << o->nome << " (" << motivo << ")\n";
}

inline void INSTRUIR(const Recurso* m) {
    if (m) std::cout << "INSTRUIR " << m->nome << "\n";
}
inline void INDICAR(const Recurso* o) {
    if (!o) return;
    if (std::find(abertos.begin(), abertos.end(), o) == abertos.end()) abertos.push_back(o);
    std::cout << "INDICAR " << o->nome << "\n";
}
inline void PARAR_INDICACAO(const Recurso* o) {
    if (!o) return;
    abertos.erase(std::remove(abertos.begin(), abertos.end(), o), abertos.end());
    std::cout << "PARAR_INDICACAO " << o->nome << "\n";
}
inline void ALERTAR(int i) { std::cout << "ALERTAR " << INTENSIDADE[i] << "\n"; }
inline void REGISTRAR(int e) { std::cout << "REGISTRAR " << EVENTO[e] << "\n"; }
inline void ESPERAR(int64_t d) {
    std::cout << "ESPERAR " << d << "s\n";
    avancar(d);
}

// Uma resposta por linha: "CONFIRMADO 3". Fim da entrada: ninguém respondeu.
inline bool ler_resposta(int& r, int64_t& t) {
    std::string linha;
    while (std::getline(std::cin, linha)) {
        std::istringstream in(linha);
        std::string nome;
        if (!(in >> nome) || nome[0] == '#') continue;
        t = 0;
        in >> t;
        for (int k = 0; k < 3; k++) if (nome == RESULTADO[k]) { r = k; return true; }
        std::cerr << "resposta desconhecida: " << nome << " (use CONFIRMADO, AJUDA ou TEMPO_ESGOTADO)\n";
        r = 2;
        return true;
    }
    return false;
}
inline int AGUARDAR_CONFIRMACAO(int64_t limite) {
    int r = 2;
    int64_t t = limite;
    int rr;
    int64_t tt;
    if (limite > 0 && ler_resposta(rr, tt) && rr != 2 && tt >= 0 && tt <= limite) { r = rr; t = tt; }
    ultima = t;
    std::cout << "AGUARDAR_CONFIRMACAO " << limite << "s -> " << RESULTADO[r] << " (" << t << "s)\n";
    avancar(t);
    return r;
}
inline Data AGORA() { return Data{true, agora}; }
inline int64_t TEMPO_DE_RESPOSTA() { return ultima; }

// Apaga as indicações ao sair, inclusive por retorna; ou erro.
struct Etapa {
    explicit Etapa(const char* nome) { std::cout << "ETAPA " << nome << "\n"; }
    ~Etapa() { fechar("fim da etapa"); }
};

inline bool rotina(const char* nome, Data horario, void (*corpo)()) {
    inicio = agora = horario.completa ? horario.seg : DIA_BASE + horario.seg;
    ultima = 0;
    passos = 0;
    abertos.clear();
    std::cout << "ROTINA " << nome << "\n";
    std::string fim = "concluida";
    bool erro = false;
    try {
        corpo();
    } catch (const Cancelar&) {
        fim = "cancelada";
    } catch (const Erro& e) {
        fim = std::string("erro: ") + e.motivo;
        erro = true;
    }
    fechar("fim da rotina");
    std::cout << "FIM " << fim << "\n";
    return erro;
}

}  // namespace qjo
"#;

/// Literal de string C++ (UTF-8 passa direto; controles viram escapes octais).
fn lit(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '?' => o.push_str("\\?"), // evita trígrafos
            c if (c as u32) < 32 || c as u32 == 127 => {
                let _ = write!(o, "\\{:03o}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

struct Gerador<'a> {
    info: &'a Info,
    out: String,
    ind: usize,
    em_rotina: bool,
    ret: Tipo,
}

fn ctipo(t: Tipo) -> &'static str {
    match t {
        Tipo::Num => "int32_t",
        Tipo::Logico => "bool",
        Tipo::Texto => "std::string",
        Tipo::Duracao => "int64_t",
        Tipo::Data => "qjo::Data",
        Tipo::Objeto | Tipo::Mensagem => "const qjo::Recurso*",
        Tipo::Enum(_) => "int",
        _ => "void",
    }
}

/// Tira um par de parênteses que envolve a expressão inteira (para `if (…)` não ficar `if ((…))`).
fn sem_parens(s: String) -> String {
    let b = s.as_bytes();
    if b.first() != Some(&b'(') || b.last() != Some(&b')') {
        return s;
    }
    let mut nivel = 0i32;
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'(' => nivel += 1,
            b')' => {
                nivel -= 1;
                if nivel == 0 && i != b.len() - 1 {
                    return s;
                }
            }
            _ => {}
        }
    }
    s[1..s.len() - 1].to_string()
}

fn tem_chamada(e: &Expr) -> bool {
    match &e.k {
        ExprK::Chamada { .. } => true,
        ExprK::Unario { e, .. } => tem_chamada(e),
        ExprK::Binario { l, r, .. } => tem_chamada(l) || tem_chamada(r),
        _ => false,
    }
}

impl<'a> Gerador<'a> {
    fn linha(&mut self, s: &str) {
        for _ in 0..self.ind {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn data(d: Data) -> String {
        format!("qjo::Data{{{}, {}LL}}", d.completa, d.seg)
    }

    /// Expressão para um destino de tipo `alvo` (decide o que `nulo` vira).
    fn para(&self, e: &Expr, alvo: Tipo) -> String {
        if let ExprK::Nulo = e.k {
            return if alvo == Tipo::Enum(RESULTADO) {
                "-1".into()
            } else {
                "nullptr".into()
            };
        }
        self.expr(e)
    }

    fn expr(&self, e: &Expr) -> String {
        match &e.k {
            ExprK::Num(n) => n.to_string(),
            ExprK::Logico(b) => b.to_string(),
            ExprK::Texto(s) => format!("std::string({})", lit(s)),
            ExprK::Duracao(s) => format!("int64_t({s})"),
            ExprK::Data(d) => Self::data(d.valor().unwrap_or(Data {
                completa: false,
                seg: 0,
            })),
            ExprK::Nulo => "nullptr".into(),
            ExprK::Nome { nome, res } => match res {
                Res::Local(_) => format!("v_{nome}"),
                Res::Global(g) => match self.info.globais[*g as usize].valor {
                    ValorGlobal::Recurso(_) => format!("&g_{nome}"),
                    _ => format!("g_{nome}"),
                },
                Res::EnumVal(_, v) => format!("{v} /*{nome}*/"),
                _ => "0".into(),
            },
            ExprK::Chamada {
                nome, args, res, ..
            } => {
                let (f, params): (String, Vec<Tipo>) = match res {
                    Res::Prim(p) => (format!("qjo::{}", p.nome()), p.params().to_vec()),
                    Res::Funcao(i) => (
                        format!("f_{nome}"),
                        self.info.funcoes[*i as usize].params.clone(),
                    ),
                    _ => return "0".into(),
                };
                let vals: Vec<String> = args
                    .iter()
                    .zip(params.iter())
                    .map(|(a, t)| self.para(a, *t))
                    .collect();
                if args.iter().filter(|a| tem_chamada(a)).count() >= 2 {
                    // O C++ não garante a ordem dos argumentos: avalia em temporários.
                    let mut s = String::from("([&] { ");
                    for (k, v) in vals.iter().enumerate() {
                        let _ = write!(s, "auto _a{k} = {v}; ");
                    }
                    let nomes: Vec<String> = (0..vals.len()).map(|k| format!("_a{k}")).collect();
                    let _ = write!(s, "return {f}({}); }}())", nomes.join(", "));
                    s
                } else {
                    format!("{f}({})", vals.join(", "))
                }
            }
            ExprK::Unario { op, e } => match op {
                OpUn::Nao => format!("(!{})", self.expr(e)),
                OpUn::Menos => format!("qjo::neg({})", self.expr(e)),
            },
            ExprK::Binario { op, l, r, .. } => {
                let (tl, tr) = (l.tipo, r.tipo);
                let (a, b) = (self.para(l, tr), self.para(r, tl));
                if matches!(op, OpBin::E | OpBin::Ou) {
                    return format!("({a} {} {b})", op.simbolo());
                }
                if tem_chamada(l) && tem_chamada(r) {
                    let corpo = self.op(*op, tl, tr, "_l", "_r");
                    return format!("([&] {{ auto _l = {a}; auto _r = {b}; return {corpo}; }}())");
                }
                self.op(*op, tl, tr, &a, &b)
            }
        }
    }

    fn op(&self, op: OpBin, tl: Tipo, tr: Tipo, a: &str, b: &str) -> String {
        use OpBin::*;
        let data = tl == Tipo::Data;
        let dur = tl == Tipo::Duracao || tr == Tipo::Duracao;
        match op {
            Igual | Diferente if data => format!("(qjo::cmp({a}, {b}) {} 0)", op.simbolo()),
            Igual | Diferente => format!("({a} {} {b})", op.simbolo()),
            Menor | Maior | MenorIgual | MaiorIgual if data => {
                format!("(qjo::cmp({a}, {b}) {} 0)", op.simbolo())
            }
            Menor | Maior | MenorIgual | MaiorIgual => format!("({a} {} {b})", op.simbolo()),
            Soma | Sub if dur => format!("qjo::dur({a} {} {b})", op.simbolo()),
            Mul if dur => format!("qjo::dur(int64_t({a}) * int64_t({b}))"),
            Div if dur => format!("qjo::dur_div({a}, {b})"),
            Soma | Sub | Mul => format!("qjo::num(int64_t({a}) {} int64_t({b}))", op.simbolo()),
            Div => format!("qjo::div({a}, {b})"),
            Resto => format!("qjo::resto({a}, {b})"),
            E | Ou => format!("({a} {} {b})", op.simbolo()),
        }
    }

    fn bloco(&mut self, b: &Bloco) {
        for s in &b.cmds {
            self.comando(s);
        }
    }

    fn sub_bloco(&mut self, cabeca: &str, b: &Bloco, rabo: &str) {
        self.linha(&format!("{cabeca}{{"));
        self.ind += 1;
        self.bloco(b);
        self.ind -= 1;
        self.linha(&format!("}}{rabo}"));
    }

    fn comando(&mut self, s: &Stmt) {
        match &s.k {
            StmtK::Decl { nome, valor, t, .. } => {
                let v = self.para(valor, *t);
                self.linha(&format!("[[maybe_unused]] {} v_{nome} = {v};", ctipo(*t)));
            }
            StmtK::Atrib {
                nome, valor, res, ..
            } => {
                let _ = res;
                let v = self.para(valor, valor.tipo);
                self.linha(&format!("v_{nome} = {v};"));
            }
            StmtK::Expr(e) => {
                let v = self.expr(e);
                self.linha(&format!("{v};"));
            }
            StmtK::Se { ramos, senao } => {
                for (i, (c, b)) in ramos.iter().enumerate() {
                    let c = sem_parens(self.expr(c));
                    let cab = if i == 0 {
                        format!("if ({c}) ")
                    } else {
                        format!("else if ({c}) ")
                    };
                    self.sub_bloco(&cab, b, "");
                }
                if let Some(b) = senao {
                    self.sub_bloco("else ", b, "");
                }
            }
            StmtK::Enquanto { cond, corpo } => {
                let c = sem_parens(self.expr(cond));
                self.linha(&format!("while ({c}) {{"));
                self.ind += 1;
                self.linha("qjo::passo();");
                self.bloco(corpo);
                self.ind -= 1;
                self.linha("}");
            }
            StmtK::Faca { corpo, cond } => {
                self.linha("do {");
                self.ind += 1;
                self.linha("qjo::passo();");
                self.bloco(corpo);
                self.ind -= 1;
                let c = sem_parens(self.expr(cond));
                self.linha(&format!("}} while ({c});"));
            }
            StmtK::Para {
                var,
                ini,
                cond,
                upd,
                corpo,
                ..
            } => {
                self.linha("{");
                self.ind += 1;
                let i = self.expr(ini);
                self.linha(&format!("[[maybe_unused]] int32_t v_{var} = {i};"));
                let c = sem_parens(self.expr(cond));
                self.linha(&format!("while ({c}) {{"));
                self.ind += 1;
                self.linha("qjo::passo();");
                self.sub_bloco("", corpo, "");
                let u = self.expr(upd);
                self.linha(&format!("v_{var} = {u};"));
                self.ind -= 1;
                self.linha("}");
                self.ind -= 1;
                self.linha("}");
            }
            StmtK::Retorna { valor } => {
                if self.em_rotina {
                    self.linha("throw qjo::Cancelar{};");
                } else {
                    match valor {
                        Some(v) => {
                            let e = self.para(v, self.ret);
                            self.linha(&format!("return {e};"));
                        }
                        None => self.linha("return;"),
                    }
                }
            }
            StmtK::Etapa { nome, corpo, .. } => {
                self.linha("{");
                self.ind += 1;
                self.linha(&format!("qjo::Etapa _etapa({});", lit(nome)));
                self.bloco(corpo);
                self.ind -= 1;
                self.linha("}");
            }
        }
    }
}

/// Gera o programa C++. `arquivo` aparece só no comentário do topo.
pub fn gerar(prog: &Programa, info: &Info, arquivo: &str) -> String {
    let mut g = Gerador {
        info,
        out: String::new(),
        ind: 0,
        em_rotina: false,
        ret: Tipo::Vazio,
    };
    let _ = writeln!(
        g.out,
        "// Gerado por qjo {} a partir de {arquivo}.",
        env!("CARGO_PKG_VERSION")
    );
    let _ = writeln!(
        g.out,
        "// Compile com: g++ -std=c++17 -O2 -o rotina rotina.cpp"
    );
    let _ = writeln!(
        g.out,
        "// Respostas pela entrada padrão, uma por linha: CONFIRMADO 3"
    );
    let rt = RUNTIME
        .replace("DIA_BASE", &DIA_BASE.to_string())
        .replace("LIMITE_ROTINA", &LIMITE_ROTINA.to_string())
        .replace("LIMITE_PASSOS", &LIMITE_PASSOS.to_string());
    g.out.push_str(&rt);
    g.out.push('\n');

    for gl in &info.globais {
        match &gl.valor {
            ValorGlobal::Recurso(d) => g.linha(&format!(
                "[[maybe_unused]] static const qjo::Recurso g_{}{{{}, {}}};",
                gl.nome,
                lit(&gl.nome),
                lit(d)
            )),
            ValorGlobal::Data(d) => g.linha(&format!(
                "[[maybe_unused]] static const qjo::Data g_{} = {};",
                gl.nome,
                Gerador::data(*d)
            )),
            ValorGlobal::Invalido => {}
        }
    }
    g.out.push('\n');

    let funcoes: Vec<&Funcao> = prog
        .itens
        .iter()
        .filter_map(|i| {
            if let Item::Funcao(f) = i {
                Some(f)
            } else {
                None
            }
        })
        .collect();
    let assinatura = |f: &Funcao| {
        let ps: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("[[maybe_unused]] {} v_{}", ctipo(p.t), p.nome))
            .collect();
        format!(
            "[[maybe_unused]] static {} f_{}({})",
            ctipo(f.ret_t),
            f.nome,
            ps.join(", ")
        )
    };
    for f in &funcoes {
        g.linha(&format!("{};", assinatura(f)));
    }
    if !funcoes.is_empty() {
        g.out.push('\n');
    }
    for f in &funcoes {
        g.em_rotina = false;
        g.ret = f.ret_t;
        g.linha(&format!("{} {{", assinatura(f)));
        g.ind += 1;
        g.bloco(&f.corpo);
        if f.ret_t != Tipo::Vazio {
            g.linha(&format!(
                "return {}{{}};  // inalcançável: todo caminho retorna",
                ctipo(f.ret_t)
            ));
        }
        g.ind -= 1;
        g.linha("}");
        g.out.push('\n');
    }

    let rotinas: Vec<&Rotina> = prog
        .itens
        .iter()
        .filter_map(|i| {
            if let Item::Rotina(r) = i {
                Some(r)
            } else {
                None
            }
        })
        .collect();
    for (k, r) in rotinas.iter().enumerate() {
        g.em_rotina = true;
        g.linha(&format!("static void rotina_{k}() {{"));
        g.ind += 1;
        g.bloco(&r.corpo);
        g.ind -= 1;
        g.linha("}");
        g.out.push('\n');
    }

    g.linha("int main() {");
    g.ind += 1;
    g.linha("bool erro = false;");
    for (k, r) in rotinas.iter().enumerate() {
        let h = g.expr(&r.horario);
        g.linha(&format!(
            "erro |= qjo::rotina({}, {h}, rotina_{k});",
            lit(&r.nome)
        ));
    }
    g.linha("std::cout.flush();");
    g.linha("return erro ? 1 : 0;");
    g.ind -= 1;
    g.linha("}");
    g.out
}
