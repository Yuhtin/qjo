//! Análise semântica (spec/03-semantica.md). Preenche tipos, resoluções e slots na árvore
//! e coleta símbolos para as ferramentas de editor.

use std::collections::HashMap;

use crate::ast::*;
use crate::diag::{Diagnostico, Fase, Linhas, Sugestao, mais_parecido};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Especie {
    Recurso,
    Horario,
    Global,
    Enum,
    ValorEnum,
    Funcao,
    Primitiva,
    Local,
    Parametro,
}

#[derive(Clone, Debug)]
pub struct Simbolo {
    pub nome: String,
    pub especie: Especie,
    pub tipo: Tipo,
    /// Onde foi declarado (nada para primitivas e valores predefinidos).
    pub def: Option<Span>,
    /// Trecho em que o nome é visível.
    pub visivel: Span,
    /// Texto curto para dicas: assinatura, descrição do recurso etc.
    pub detalhe: String,
}

#[derive(Clone, Debug)]
pub enum ValorGlobal {
    Recurso(String),
    Data(Data),
    Invalido,
}

#[derive(Clone, Debug)]
pub struct Global {
    pub nome: String,
    pub tipo: Tipo,
    pub valor: ValorGlobal,
}

#[derive(Clone, Debug)]
pub struct EnumInfo {
    pub nome: String,
    pub valores: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Assinatura {
    pub nome: String,
    pub params: Vec<Tipo>,
    pub ret: Tipo,
    /// Índice do item no programa.
    pub item: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub globais: Vec<Global>,
    pub enums: Vec<EnumInfo>,
    pub funcoes: Vec<Assinatura>,
    pub simbolos: Vec<Simbolo>,
    /// Usos de nomes: trecho e símbolo.
    pub usos: Vec<(Span, usize)>,
}

impl Info {
    pub fn nome_tipo(&self, t: Tipo) -> String {
        match t {
            Tipo::Num => "num".into(),
            Tipo::Logico => "logico".into(),
            Tipo::Texto => "texto".into(),
            Tipo::Duracao => "duracao".into(),
            Tipo::Data => "data".into(),
            Tipo::Objeto => "objeto".into(),
            Tipo::Mensagem => "mensagem".into(),
            Tipo::Enum(i) => self
                .enums
                .get(i as usize)
                .map(|e| e.nome.clone())
                .unwrap_or_default(),
            Tipo::Vazio => "sem valor".into(),
            Tipo::Nulo => "nulo".into(),
            Tipo::Erro => "?".into(),
        }
    }
}

#[derive(Clone, Copy)]
enum GDef {
    Res(Res),
    TipoEnum(u16),
}

struct Local {
    nome: String,
    slot: u32,
    tipo: Tipo,
    simb: usize,
}

struct Ctx {
    retorno: Option<Tipo>,
    em_rotina: bool,
    em_etapa: bool,
    slots: u32,
    funcao: Option<u32>,
}

struct Sema<'a> {
    linhas: Linhas<'a>,
    diags: Vec<Diagnostico>,
    info: Info,
    globais: HashMap<String, (GDef, usize)>,
    escopos: Vec<Vec<Local>>,
    ctx: Ctx,
    chamadas: Vec<Vec<(u32, Span)>>,
}

const MAX_DUR: u64 = i32::MAX as u64;

fn nome_tipo_base(n: &str) -> Option<Tipo> {
    Some(match n {
        "num" => Tipo::Num,
        "logico" => Tipo::Logico,
        "texto" => Tipo::Texto,
        "duracao" => Tipo::Duracao,
        "data" => Tipo::Data,
        "objeto" => Tipo::Objeto,
        "mensagem" => Tipo::Mensagem,
        "resultado" => Tipo::Enum(RESULTADO),
        "intensidade" => Tipo::Enum(INTENSIDADE),
        "evento" => Tipo::Enum(EVENTO),
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Const {
    Num(i64),
    Dur(i64),
}

impl<'a> Sema<'a> {
    fn erro(
        &mut self,
        codigo: &'static str,
        span: Span,
        mensagem: String,
        sugestao: Option<Sugestao>,
    ) {
        let (linha, coluna) = self.linhas.posicao(span.ini as usize);
        self.diags.push(Diagnostico {
            codigo,
            mensagem,
            ini: span.ini as usize,
            fim: span.fim as usize,
            linha,
            coluna,
            fase: Fase::Semantica,
            sugestao,
        });
    }

    fn nt(&self, t: Tipo) -> String {
        self.info.nome_tipo(t)
    }

    fn simbolo(
        &mut self,
        nome: &str,
        especie: Especie,
        tipo: Tipo,
        def: Option<Span>,
        visivel: Span,
        detalhe: String,
    ) -> usize {
        self.info.simbolos.push(Simbolo {
            nome: nome.to_string(),
            especie,
            tipo,
            def,
            visivel,
            detalhe,
        });
        self.info.simbolos.len() - 1
    }

    fn uso(&mut self, span: Span, simb: usize) {
        self.info.usos.push((span, simb));
    }

    fn tudo() -> Span {
        Span::new(0, u32::MAX)
    }

    /// Procura um nome visível: locais (de dentro para fora) e depois o topo.
    fn procura(&self, nome: &str) -> Option<(GDef, usize, Tipo)> {
        for esc in self.escopos.iter().rev() {
            if let Some(l) = esc.iter().find(|l| l.nome == nome) {
                return Some((GDef::Res(Res::Local(l.slot)), l.simb, l.tipo));
            }
        }
        self.globais
            .get(nome)
            .map(|(d, s)| (*d, *s, self.info.simbolos[*s].tipo))
    }

    fn nomes_visiveis(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .escopos
            .iter()
            .flatten()
            .map(|l| l.nome.clone())
            .collect();
        v.extend(self.globais.keys().cloned());
        v
    }

    fn sugestao_nome(&self, nome: &str, span: Span, candidatos: &[String]) -> Option<Sugestao> {
        mais_parecido(nome, candidatos.iter().map(String::as_str)).map(|c| {
            Sugestao::troca(
                format!("Você quis dizer {c}?"),
                span.ini as usize,
                span.fim as usize,
                c,
            )
        })
    }

    fn redecl(&mut self, nome: &str, span: Span) -> bool {
        if let Some((_, s, _)) = self.procura(nome) {
            let esp = match self.info.simbolos[s].especie {
                Especie::Primitiva => "uma primitiva",
                Especie::Funcao => "uma função",
                Especie::Recurso => "um recurso",
                Especie::Horario => "um horário",
                Especie::Enum => "um enum",
                Especie::ValorEnum => "um valor de enum",
                Especie::Parametro => "um parâmetro",
                Especie::Local | Especie::Global => "uma variável",
            };
            self.erro(
                "E_SEM_REDECL",
                span,
                format!("\"{nome}\" já existe ({esp}); escolha outro nome"),
                None,
            );
            return true;
        }
        false
    }

    fn resolve_tipo(&mut self, t: &TipoSint) -> Tipo {
        if t.nome == "enum" {
            self.erro(
                "E_SEM_TIPO",
                t.span,
                "\"enum\" só declara conjuntos no topo: enum Nome { A, B }".into(),
                None,
            );
            return Tipo::Erro;
        }
        if let Some(b) = nome_tipo_base(&t.nome) {
            return b;
        }
        match self.globais.get(&t.nome).copied() {
            Some((GDef::TipoEnum(i), s)) => {
                self.uso(t.span, s);
                Tipo::Enum(i)
            }
            _ => {
                let cands: Vec<String> = self
                    .info
                    .enums
                    .iter()
                    .skip(3)
                    .map(|e| e.nome.clone())
                    .collect();
                let sug = self.sugestao_nome(&t.nome, t.span, &cands);
                self.erro(
                    "E_SEM_TIPO",
                    t.span,
                    format!("o tipo \"{}\" não existe", t.nome),
                    sug,
                );
                Tipo::Erro
            }
        }
    }

    fn compativel(destino: Tipo, valor: Tipo) -> bool {
        destino == Tipo::Erro
            || valor == Tipo::Erro
            || destino == valor
            || (valor == Tipo::Nulo && destino.aceita_nulo())
    }

    // ---------- topo ----------

    fn declarar_topo(&mut self, prog: &mut Programa) {
        let predef = [
            (
                "resultado",
                ["CONFIRMADO", "AJUDA", "TEMPO_ESGOTADO"].as_slice(),
            ),
            ("intensidade", ["BAIXA", "MEDIA", "ALTA"].as_slice()),
            (
                "evento",
                [
                    "ETAPA_PULADA",
                    "AJUDA_REPETIDA",
                    "CONFIRMACAO_RAPIDA",
                    "ROTINA_INCOMPLETA",
                ]
                .as_slice(),
            ),
        ];
        for (i, (nome, vals)) in predef.iter().enumerate() {
            self.info.enums.push(EnumInfo {
                nome: nome.to_string(),
                valores: vals.iter().map(|v| v.to_string()).collect(),
            });
            for (j, v) in vals.iter().enumerate() {
                let s = self.simbolo(
                    v,
                    Especie::ValorEnum,
                    Tipo::Enum(i as u16),
                    None,
                    Self::tudo(),
                    format!("valor de {nome}"),
                );
                self.globais.insert(
                    v.to_string(),
                    (GDef::Res(Res::EnumVal(i as u16, j as u16)), s),
                );
            }
        }
        for (n, p) in PRIMITIVAS {
            let params: Vec<String> = p.params().iter().map(|t| self.nt(*t)).collect();
            let ret = if p.retorno() == Tipo::Vazio {
                String::new()
            } else {
                format!(": {}", self.nt(p.retorno()))
            };
            let det = format!("{n}({}){ret}", params.join(", "));
            let s = self.simbolo(n, Especie::Primitiva, p.retorno(), None, Self::tudo(), det);
            self.globais
                .insert(n.to_string(), (GDef::Res(Res::Prim(p)), s));
        }

        // Enums primeiro: os tipos declarados podem aparecer em qualquer assinatura.
        for item in prog.itens.iter() {
            if let Item::Enum {
                nome,
                nome_span,
                valores,
                span,
            } = item
            {
                let idx = self.info.enums.len() as u16;
                if !self.redecl(nome, *nome_span) {
                    let s = self.simbolo(
                        nome,
                        Especie::Enum,
                        Tipo::Enum(idx),
                        Some(*nome_span),
                        Self::tudo(),
                        format!("enum {nome}"),
                    );
                    self.globais.insert(nome.clone(), (GDef::TipoEnum(idx), s));
                }
                let mut vals = vec![];
                for (v, vs) in valores {
                    if vals.contains(v) {
                        self.erro(
                            "E_SEM_REDECL",
                            *vs,
                            format!("o valor \"{v}\" aparece duas vezes neste enum"),
                            None,
                        );
                        continue;
                    }
                    if !self.redecl(v, *vs) {
                        let s = self.simbolo(
                            v,
                            Especie::ValorEnum,
                            Tipo::Enum(idx),
                            Some(*vs),
                            Self::tudo(),
                            format!("valor de {nome}"),
                        );
                        self.globais.insert(
                            v.clone(),
                            (GDef::Res(Res::EnumVal(idx, vals.len() as u16)), s),
                        );
                    }
                    vals.push(v.clone());
                }
                let _ = span;
                self.info.enums.push(EnumInfo {
                    nome: nome.clone(),
                    valores: vals,
                });
            }
        }

        for (k, item) in prog.itens.iter_mut().enumerate() {
            match item {
                Item::Global {
                    tipo,
                    nome,
                    nome_span,
                    valor,
                    ..
                } => self.declarar_global(tipo, nome, *nome_span, valor),
                Item::Funcao(f) => {
                    let ret = match &f.ret {
                        Some(t) => self.resolve_tipo(t),
                        None => Tipo::Vazio,
                    };
                    f.ret_t = ret;
                    let mut params = vec![];
                    for p in f.params.iter_mut() {
                        p.t = self.resolve_tipo(&p.tipo);
                        params.push(p.t);
                    }
                    let idx = self.info.funcoes.len() as u32;
                    let det = format!(
                        "funcao {}{}({})",
                        if ret == Tipo::Vazio {
                            String::new()
                        } else {
                            format!("{} ", self.nt(ret))
                        },
                        f.nome,
                        f.params
                            .iter()
                            .map(|p| format!("{} {}", self.nt(p.t), p.nome))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    if !self.redecl(&f.nome, f.nome_span) {
                        let s = self.simbolo(
                            &f.nome,
                            Especie::Funcao,
                            ret,
                            Some(f.nome_span),
                            Self::tudo(),
                            det,
                        );
                        self.globais
                            .insert(f.nome.clone(), (GDef::Res(Res::Funcao(idx)), s));
                    }
                    self.info.funcoes.push(Assinatura {
                        nome: f.nome.clone(),
                        params,
                        ret,
                        item: k,
                    });
                }
                _ => {}
            }
        }
    }

    fn declarar_global(&mut self, tipo: &TipoSint, nome: &str, nome_span: Span, valor: &mut Expr) {
        let t = self.resolve_tipo(tipo);
        let (especie, v, det) = match t {
            Tipo::Objeto | Tipo::Mensagem => match &valor.k {
                ExprK::Texto(s) if !s.is_empty() => {
                    valor.tipo = Tipo::Texto;
                    (
                        Especie::Recurso,
                        ValorGlobal::Recurso(s.clone()),
                        format!("{} {nome} = \"{s}\"", self.nt(t)),
                    )
                }
                ExprK::Texto(_) => {
                    self.erro(
                        "E_SEM_RECURSO",
                        valor.span,
                        "a descrição do recurso não pode ser vazia".into(),
                        None,
                    );
                    (Especie::Recurso, ValorGlobal::Invalido, String::new())
                }
                _ => {
                    self.erro(
                        "E_SEM_TIPO",
                        valor.span,
                        format!(
                            "um {} do topo é criado com uma descrição entre aspas",
                            self.nt(t)
                        ),
                        None,
                    );
                    (Especie::Recurso, ValorGlobal::Invalido, String::new())
                }
            },
            Tipo::Data => match &valor.k {
                ExprK::Data(d) => match d.valor() {
                    Some(dv) => {
                        valor.tipo = Tipo::Data;
                        (
                            Especie::Horario,
                            ValorGlobal::Data(dv),
                            format!("data {nome}"),
                        )
                    }
                    None => {
                        self.erro(
                            "E_SEM_DATA",
                            valor.span,
                            "data impossível: confira mês, dia e horário".into(),
                            None,
                        );
                        (Especie::Horario, ValorGlobal::Invalido, String::new())
                    }
                },
                _ => {
                    self.erro(
                        "E_SEM_TIPO",
                        valor.span,
                        "um horário do topo recebe um literal data\"…\"".into(),
                        None,
                    );
                    (Especie::Horario, ValorGlobal::Invalido, String::new())
                }
            },
            Tipo::Erro => (Especie::Global, ValorGlobal::Invalido, String::new()),
            _ => {
                self.erro(
                    "E_SEM_GLOBAL",
                    tipo.span,
                    format!("no topo só ficam recursos (objeto, mensagem) e horários (data); não há variável global de {}", self.nt(t)),
                    None,
                );
                (Especie::Global, ValorGlobal::Invalido, String::new())
            }
        };
        if self.redecl(nome, nome_span) {
            return;
        }
        let idx = self.info.globais.len() as u32;
        self.info.globais.push(Global {
            nome: nome.to_string(),
            tipo: t,
            valor: v,
        });
        let s = self.simbolo(nome, especie, t, Some(nome_span), Self::tudo(), det);
        self.globais
            .insert(nome.to_string(), (GDef::Res(Res::Global(idx)), s));
    }

    // ---------- corpos ----------

    fn novo_slot(&mut self) -> u32 {
        self.ctx.slots += 1;
        self.ctx.slots - 1
    }

    fn declarar_local(
        &mut self,
        nome: &str,
        span: Span,
        tipo: Tipo,
        visivel: Span,
        especie: Especie,
    ) -> u32 {
        let slot = self.novo_slot();
        let det = format!("{} {nome}", self.nt(tipo));
        let simb = self.simbolo(nome, especie, tipo, Some(span), visivel, det);
        self.escopos.last_mut().unwrap().push(Local {
            nome: nome.to_string(),
            slot,
            tipo,
            simb,
        });
        slot
    }

    fn funcao(&mut self, f: &mut Funcao, idx: u32) {
        self.ctx = Ctx {
            retorno: Some(f.ret_t),
            em_rotina: false,
            em_etapa: false,
            slots: 0,
            funcao: Some(idx),
        };
        self.escopos.push(vec![]);
        let vis = f.corpo.span;
        for p in f.params.iter() {
            if !self.redecl(&p.nome, p.nome_span) {
                self.declarar_local(&p.nome, p.nome_span, p.t, vis, Especie::Parametro);
            } else {
                self.novo_slot();
            }
        }
        self.bloco(&mut f.corpo);
        self.escopos.pop();
        f.nslots = self.ctx.slots;
        if f.ret_t != Tipo::Vazio && f.ret_t != Tipo::Erro && !sempre_retorna(&f.corpo) {
            let m = format!(
                "a função {} devolve {}, mas nem todo caminho termina em \"retorna valor;\"",
                f.nome,
                self.nt(f.ret_t)
            );
            self.erro("E_SEM_RETORNO", f.nome_span, m, None);
        }
    }

    fn rotina(&mut self, r: &mut Rotina) {
        self.ctx = Ctx {
            retorno: None,
            em_rotina: true,
            em_etapa: false,
            slots: 0,
            funcao: None,
        };
        if r.nome.trim().is_empty() {
            self.erro(
                "E_SEM_NOME_VAZIO",
                r.nome_span,
                "o nome da rotina não pode ser vazio".into(),
                None,
            );
        }
        let t = self.expr(&mut r.horario);
        let ok = matches!(
            &r.horario.k,
            ExprK::Data(_)
                | ExprK::Nome {
                    res: Res::Global(_),
                    ..
                }
        );
        if t != Tipo::Erro && (t != Tipo::Data || !ok) {
            let m = format!(
                "o horário da rotina é um horário do topo ou um literal data\"…\"; veio {}",
                self.nt(t)
            );
            self.erro("E_SEM_TIPO", r.horario.span, m, None);
        }
        self.bloco(&mut r.corpo);
        r.nslots = self.ctx.slots;
    }

    fn bloco(&mut self, b: &mut Bloco) {
        self.escopos.push(vec![]);
        let fim = b.span.fim;
        for s in b.cmds.iter_mut() {
            self.comando(s, fim);
        }
        self.escopos.pop();
    }

    fn condicao(&mut self, e: &mut Expr, onde: &str) {
        let t = self.expr(e);
        if t != Tipo::Logico && t != Tipo::Erro {
            let m = format!("a condição {onde} precisa ser logico, veio {}", self.nt(t));
            self.erro("E_SEM_TIPO", e.span, m, None);
        }
    }

    fn comando(&mut self, s: &mut Stmt, fim_bloco: u32) {
        let span = s.span;
        match &mut s.k {
            StmtK::Decl {
                tipo,
                nome,
                nome_span,
                valor,
                slot,
                t,
            } => {
                let tt = self.resolve_tipo(tipo);
                *t = tt;
                let tv = self.expr(valor);
                if matches!(tt, Tipo::Objeto | Tipo::Mensagem) && matches!(valor.k, ExprK::Texto(_))
                {
                    self.erro(
                        "E_SEM_RECURSO",
                        valor.span,
                        "recursos com descrição ficam no topo; aqui só cabe um recurso existente ou nulo".into(),
                        None,
                    );
                } else if !Self::compativel(tt, tv) {
                    let m = format!("\"{nome}\" é {}, mas recebeu {}", self.nt(tt), self.nt(tv));
                    self.erro("E_SEM_TIPO", valor.span, m, None);
                }
                if self.redecl(nome, *nome_span) {
                    *slot = self.novo_slot();
                } else {
                    *slot = self.declarar_local(
                        nome,
                        *nome_span,
                        tt,
                        Span::new(span.fim, fim_bloco),
                        Especie::Local,
                    );
                }
            }
            StmtK::Atrib {
                nome,
                nome_span,
                valor,
                res,
            } => {
                let tv = self.expr(valor);
                match self.procura(nome) {
                    Some((GDef::Res(r @ Res::Local(_)), simb, t)) => {
                        *res = r;
                        self.uso(*nome_span, simb);
                        if tv == Tipo::Nulo && t.aceita_nulo() {
                            valor.tipo = t; // o gerador C++ precisa saber o que nulo vira
                        }
                        if !Self::compativel(t, tv) {
                            let m =
                                format!("\"{nome}\" é {}, mas recebeu {}", self.nt(t), self.nt(tv));
                            self.erro("E_SEM_TIPO", valor.span, m, None);
                        }
                    }
                    Some((_, simb, _)) => {
                        self.uso(*nome_span, simb);
                        let o_que = match self.info.simbolos[simb].especie {
                            Especie::Recurso | Especie::Horario => "é constante",
                            Especie::Funcao | Especie::Primitiva => "é uma função",
                            Especie::ValorEnum => "é um valor de enum",
                            Especie::Enum => "é um tipo",
                            _ => "não é uma variável",
                        };
                        self.erro(
                            "E_SEM_ATRIB",
                            *nome_span,
                            format!("não dá para atribuir a \"{nome}\": {o_que}"),
                            None,
                        );
                    }
                    None => {
                        let cands = self.nomes_visiveis();
                        let sug = self.sugestao_nome(nome, *nome_span, &cands);
                        self.erro(
                            "E_SEM_NOME",
                            *nome_span,
                            format!("\"{nome}\" não foi declarado"),
                            sug,
                        );
                    }
                }
            }
            StmtK::Expr(e) => {
                self.expr(e);
                if !matches!(e.k, ExprK::Chamada { .. }) {
                    let m = "esta expressão sozinha não faz nada; um comando é uma chamada, uma atribuição ou uma declaração";
                    self.erro("E_SEM_CMD", e.span, m.into(), None);
                }
            }
            StmtK::Se { ramos, senao } => {
                for (i, (c, b)) in ramos.iter_mut().enumerate() {
                    self.condicao(c, if i == 0 { "do q?" } else { "do qjo?" });
                    self.bloco(b);
                }
                if let Some(b) = senao {
                    self.bloco(b);
                }
            }
            StmtK::Enquanto { cond, corpo } => {
                self.condicao(cond, "do enquanto");
                self.bloco(corpo);
            }
            StmtK::Faca { corpo, cond } => {
                self.bloco(corpo);
                self.condicao(cond, "do faca … enquanto");
            }
            StmtK::Para {
                tipo,
                var,
                var_span,
                ini,
                cond,
                alvo,
                alvo_span,
                upd,
                corpo,
                slot,
            } => {
                let tt = self.resolve_tipo(tipo);
                if tt != Tipo::Num && tt != Tipo::Erro {
                    let m = format!("a variável do \"para\" é um num novo, veio {}", self.nt(tt));
                    self.erro("E_SEM_PARA", tipo.span, m, None);
                }
                let ti = self.expr(ini);
                if !Self::compativel(Tipo::Num, ti) {
                    let m = format!(
                        "o valor inicial do \"para\" precisa ser num, veio {}",
                        self.nt(ti)
                    );
                    self.erro("E_SEM_TIPO", ini.span, m, None);
                }
                self.escopos.push(vec![]);
                if self.redecl(var, *var_span) {
                    *slot = self.novo_slot();
                } else {
                    *slot = self.declarar_local(
                        var,
                        *var_span,
                        Tipo::Num,
                        Span::new(var_span.fim, corpo.span.fim),
                        Especie::Local,
                    );
                }
                self.condicao(cond, "do para");
                if alvo != var {
                    let m = format!(
                        "a atualização do \"para\" precisa atribuir à variável do laço \"{var}\""
                    );
                    let sug = Sugestao::troca(
                        format!("Trocar por {var}"),
                        alvo_span.ini as usize,
                        alvo_span.fim as usize,
                        var.clone(),
                    );
                    self.erro("E_SEM_PARA", *alvo_span, m, Some(sug));
                } else if let Some((_, simb, _)) = self.procura(var) {
                    self.uso(*alvo_span, simb);
                }
                let tu = self.expr(upd);
                if !Self::compativel(Tipo::Num, tu) {
                    let m = format!(
                        "a atualização do \"para\" precisa dar num, veio {}",
                        self.nt(tu)
                    );
                    self.erro("E_SEM_TIPO", upd.span, m, None);
                }
                self.bloco(corpo);
                self.escopos.pop();
            }
            StmtK::Retorna { valor } => {
                let tv = valor.as_mut().map(|v| (self.expr(v), v.span));
                match (self.ctx.retorno, tv) {
                    (None, Some((_, sp))) => {
                        self.erro(
                            "E_SEM_RETORNO",
                            sp,
                            "na rotina, \"retorna;\" encerra sem valor".into(),
                            None,
                        );
                    }
                    (Some(Tipo::Vazio), Some((_, sp))) => {
                        self.erro(
                            "E_SEM_RETORNO",
                            sp,
                            "função sem tipo de retorno usa só \"retorna;\"".into(),
                            None,
                        );
                    }
                    (Some(rt), None) if rt != Tipo::Vazio && rt != Tipo::Erro => {
                        let m = format!(
                            "esta função devolve {}: use \"retorna valor;\"",
                            self.nt(rt)
                        );
                        self.erro("E_SEM_RETORNO", span, m, None);
                    }
                    (Some(rt), Some((t, sp))) if !Self::compativel(rt, t) => {
                        let m = format!(
                            "a função devolve {}, mas este retorna dá {}",
                            self.nt(rt),
                            self.nt(t)
                        );
                        self.erro("E_SEM_TIPO", sp, m, None);
                    }
                    _ => {}
                }
            }
            StmtK::Etapa {
                nome,
                nome_span,
                corpo,
            } => {
                if !self.ctx.em_rotina {
                    self.erro(
                        "E_SEM_ETAPA",
                        span,
                        "etapa não pode ficar dentro de função".into(),
                        None,
                    );
                } else if self.ctx.em_etapa {
                    self.erro(
                        "E_SEM_ETAPA",
                        span,
                        "etapa não pode ficar dentro de outra etapa".into(),
                        None,
                    );
                }
                if nome.trim().is_empty() {
                    self.erro(
                        "E_SEM_NOME_VAZIO",
                        *nome_span,
                        "o nome da etapa não pode ser vazio".into(),
                        None,
                    );
                }
                let antes = self.ctx.em_etapa;
                self.ctx.em_etapa = true;
                self.bloco(corpo);
                self.ctx.em_etapa = antes;
            }
        }
    }

    // ---------- expressões ----------

    fn expr(&mut self, e: &mut Expr) -> Tipo {
        let t = self.expr_k(e);
        e.tipo = t;
        t
    }

    fn expr_k(&mut self, e: &mut Expr) -> Tipo {
        let span = e.span;
        match &mut e.k {
            ExprK::Num(_) => Tipo::Num,
            ExprK::Logico(_) => Tipo::Logico,
            ExprK::Texto(_) => Tipo::Texto,
            ExprK::Nulo => Tipo::Nulo,
            ExprK::Duracao(s) => {
                if *s > MAX_DUR {
                    self.erro(
                        "E_SEM_DURACAO",
                        span,
                        "duração acima do máximo de 2147483647 segundos".into(),
                        None,
                    );
                }
                Tipo::Duracao
            }
            ExprK::Data(d) => {
                if d.valor().is_none() {
                    self.erro(
                        "E_SEM_DATA",
                        span,
                        "data impossível: confira mês, dia e horário".into(),
                        None,
                    );
                }
                Tipo::Data
            }
            ExprK::Nome { nome, res } => match self.procura(nome) {
                Some((
                    GDef::Res(r @ (Res::Local(_) | Res::Global(_) | Res::EnumVal(..))),
                    simb,
                    t,
                )) => {
                    *res = r;
                    self.uso(span, simb);
                    t
                }
                Some((d, simb, _)) => {
                    self.uso(span, simb);
                    let m = match d {
                        GDef::TipoEnum(_) => format!("\"{nome}\" é um tipo, não um valor"),
                        _ => format!("\"{nome}\" é uma função; chame com {nome}(…)"),
                    };
                    self.erro("E_SEM_TIPO", span, m, None);
                    Tipo::Erro
                }
                None => {
                    let cands = self.nomes_visiveis();
                    let sug = self.sugestao_nome(nome, span, &cands);
                    self.erro(
                        "E_SEM_NOME",
                        span,
                        format!("\"{nome}\" não foi declarado"),
                        sug,
                    );
                    Tipo::Erro
                }
            },
            ExprK::Chamada {
                nome,
                nome_span,
                args,
                res,
            } => {
                let targs: Vec<(Tipo, Span)> =
                    args.iter_mut().map(|a| (self.expr(a), a.span)).collect();
                let (params, ret): (Vec<Tipo>, Tipo) = match self.procura(nome) {
                    Some((GDef::Res(r @ Res::Prim(p)), simb, _)) => {
                        *res = r;
                        self.uso(*nome_span, simb);
                        (p.params().to_vec(), p.retorno())
                    }
                    Some((GDef::Res(r @ Res::Funcao(i)), simb, _)) => {
                        *res = r;
                        self.uso(*nome_span, simb);
                        if let Some(f) = self.ctx.funcao {
                            self.chamadas[f as usize].push((i, *nome_span));
                        }
                        let a = &self.info.funcoes[i as usize];
                        (a.params.clone(), a.ret)
                    }
                    Some((_, simb, _)) => {
                        self.uso(*nome_span, simb);
                        self.erro(
                            "E_SEM_TIPO",
                            *nome_span,
                            format!("\"{nome}\" não é uma função"),
                            None,
                        );
                        return Tipo::Erro;
                    }
                    None => {
                        let cands: Vec<String> = PRIMITIVAS
                            .iter()
                            .map(|(n, _)| n.to_string())
                            .chain(self.info.funcoes.iter().map(|f| f.nome.clone()))
                            .collect();
                        let sug = self.sugestao_nome(nome, *nome_span, &cands);
                        self.erro(
                            "E_SEM_NOME",
                            *nome_span,
                            format!("a função \"{nome}\" não existe"),
                            sug,
                        );
                        return Tipo::Erro;
                    }
                };
                if params.len() != targs.len() {
                    let m = format!(
                        "{nome} recebe {} argumento{}, veio {}",
                        params.len(),
                        if params.len() == 1 { "" } else { "s" },
                        targs.len()
                    );
                    self.erro("E_SEM_TIPO", span, m, None);
                } else {
                    for (p, (t, sp)) in params.iter().zip(targs) {
                        if !Self::compativel(*p, t) {
                            let m = format!("{nome} espera {}, veio {}", self.nt(*p), self.nt(t));
                            self.erro("E_SEM_TIPO", sp, m, None);
                        }
                    }
                }
                ret
            }
            ExprK::Unario { op, e: inner } => {
                let op = *op;
                let t = self.expr(inner);
                match (op, t) {
                    (_, Tipo::Erro) => match op {
                        OpUn::Nao => Tipo::Logico,
                        OpUn::Menos => Tipo::Erro,
                    },
                    (OpUn::Nao, Tipo::Logico) => Tipo::Logico,
                    (OpUn::Menos, Tipo::Num) => Tipo::Num,
                    (OpUn::Nao, t) => {
                        let m = format!("\"!\" só vale para logico, veio {}", self.nt(t));
                        self.erro("E_SEM_TIPO", span, m, None);
                        Tipo::Erro
                    }
                    (OpUn::Menos, t) => {
                        let m = format!("o sinal \"-\" só vale para num, veio {}", self.nt(t));
                        self.erro("E_SEM_TIPO", span, m, None);
                        Tipo::Erro
                    }
                }
            }
            ExprK::Binario { op, l, r, op_span } => {
                let (op, op_span) = (*op, *op_span);
                let tl = self.expr(l);
                let tr = self.expr(r);
                let t = self.tipo_binario(op, tl, tr, op_span);
                if matches!(
                    op,
                    OpBin::Soma | OpBin::Sub | OpBin::Mul | OpBin::Div | OpBin::Resto
                ) && t != Tipo::Erro
                {
                    self.confere_constante(op, l, r, span);
                }
                t
            }
        }
    }

    fn tipo_binario(&mut self, op: OpBin, tl: Tipo, tr: Tipo, span: Span) -> Tipo {
        use OpBin::*;
        use Tipo::*;
        let logico = matches!(
            op,
            Ou | E | Igual | Diferente | Menor | Maior | MenorIgual | MaiorIgual
        );
        if tl == Erro || tr == Erro {
            return if logico { Logico } else { Erro };
        }
        let ok = match op {
            Ou | E => (tl == Logico && tr == Logico).then_some(Logico),
            Igual | Diferente => (tl == tr && tl != Nulo && tl != Vazio
                || (tl == Nulo && tr.aceita_nulo())
                || (tr == Nulo && tl.aceita_nulo()))
            .then_some(Logico),
            Menor | Maior | MenorIgual | MaiorIgual => {
                (tl == tr && matches!(tl, Num | Duracao | Data)).then_some(Logico)
            }
            Soma | Sub => match (tl, tr) {
                (Num, Num) => Some(Num),
                (Duracao, Duracao) => Some(Duracao),
                _ => None,
            },
            Mul => match (tl, tr) {
                (Num, Num) => Some(Num),
                (Duracao, Num) | (Num, Duracao) => Some(Duracao),
                _ => None,
            },
            Div => match (tl, tr) {
                (Num, Num) => Some(Num),
                (Duracao, Num) => Some(Duracao),
                _ => None,
            },
            Resto => (tl == Num && tr == Num).then_some(Num),
        };
        match ok {
            Some(t) => t,
            None => {
                let m = format!(
                    "\"{}\" não vale entre {} e {}",
                    op.simbolo(),
                    self.nt(tl),
                    self.nt(tr)
                );
                self.erro("E_SEM_TIPO", span, m, None);
                if logico { Logico } else { Erro }
            }
        }
    }

    fn confere_constante(&mut self, op: OpBin, l: &Expr, r: &Expr, span: Span) {
        if matches!(op, OpBin::Div | OpBin::Resto) && matches!(r.k, ExprK::Num(0)) {
            let m = if op == OpBin::Div {
                "divisão por zero"
            } else {
                "resto da divisão por zero"
            };
            self.erro("E_SEM_CONSTANTE", r.span, m.into(), None);
            return;
        }
        let (Some(a), Some(b)) = (constante(l), constante(r)) else {
            return;
        };
        if aplicar_const(op, a, b).is_none() {
            let _ = span;
            let m = match (op, b) {
                (OpBin::Div | OpBin::Resto, Const::Num(0)) => "divisão por zero".to_string(),
                _ => "o resultado desta conta sai do intervalo permitido".to_string(),
            };
            self.erro("E_SEM_CONSTANTE", span, m, None);
        }
    }

    fn recursao(&mut self) {
        let n = self.chamadas.len();
        let alcanca = |de: usize, ate: usize, ch: &Vec<Vec<(u32, Span)>>| -> bool {
            let mut visto = vec![false; n];
            let mut pilha = vec![de];
            while let Some(x) = pilha.pop() {
                if x == ate {
                    return true;
                }
                if std::mem::replace(&mut visto[x], true) {
                    continue;
                }
                pilha.extend(ch[x].iter().map(|(y, _)| *y as usize));
            }
            false
        };
        let mut erros = vec![];
        for f in 0..n {
            for (g, sp) in &self.chamadas[f] {
                if alcanca(*g as usize, f, &self.chamadas) {
                    let caminho = if *g as usize == f {
                        format!("{} chama a si mesma", self.info.funcoes[f].nome)
                    } else {
                        format!(
                            "{} → {} → … → {}",
                            self.info.funcoes[f].nome,
                            self.info.funcoes[*g as usize].nome,
                            self.info.funcoes[f].nome
                        )
                    };
                    erros.push((*sp, format!("recursão não é permitida ({caminho})")));
                }
            }
        }
        for (sp, m) in erros {
            self.erro("E_SEM_RECURSAO", sp, m, None);
        }
    }
}

fn constante(e: &Expr) -> Option<Const> {
    match &e.k {
        ExprK::Num(n) => Some(Const::Num(*n as i64)),
        ExprK::Duracao(s) if *s <= MAX_DUR => Some(Const::Dur(*s as i64)),
        ExprK::Unario { op: OpUn::Menos, e } => match constante(e)? {
            Const::Num(n) => Some(Const::Num(-n))
                .filter(|c| matches!(c, Const::Num(v) if i32::try_from(*v).is_ok())),
            _ => None,
        },
        ExprK::Binario { op, l, r, .. } => aplicar_const(*op, constante(l)?, constante(r)?),
        _ => None,
    }
}

fn aplicar_const(op: OpBin, a: Const, b: Const) -> Option<Const> {
    use Const::*;
    let num = |v: i64| i32::try_from(v).ok().map(|x| Num(x as i64));
    let dur = |v: i64| (0..=MAX_DUR as i64).contains(&v).then_some(Dur(v));
    match (op, a, b) {
        (OpBin::Soma, Num(x), Num(y)) => num(x + y),
        (OpBin::Sub, Num(x), Num(y)) => num(x - y),
        (OpBin::Mul, Num(x), Num(y)) => num(x * y),
        (OpBin::Div, Num(x), Num(y)) => (y != 0).then(|| num(x / y)).flatten(),
        (OpBin::Resto, Num(x), Num(y)) => (y != 0).then(|| num(x % y)).flatten(),
        (OpBin::Soma, Dur(x), Dur(y)) => dur(x + y),
        (OpBin::Sub, Dur(x), Dur(y)) => dur(x - y),
        (OpBin::Mul, Dur(x), Num(y)) | (OpBin::Mul, Num(y), Dur(x)) => dur(x * y),
        (OpBin::Div, Dur(x), Num(y)) => (y != 0).then(|| dur(x / y)).flatten(),
        _ => None,
    }
}

/// Todo caminho do bloco termina num `retorna`?
pub fn sempre_retorna(b: &Bloco) -> bool {
    b.cmds.iter().any(|s| match &s.k {
        StmtK::Retorna { .. } => true,
        StmtK::Se {
            ramos,
            senao: Some(sb),
        } => ramos.iter().all(|(_, rb)| sempre_retorna(rb)) && sempre_retorna(sb),
        StmtK::Faca { corpo, .. } => sempre_retorna(corpo),
        StmtK::Etapa { corpo, .. } => sempre_retorna(corpo),
        _ => false,
    })
}

pub struct Checado {
    pub info: Info,
    pub diagnosticos: Vec<Diagnostico>,
}

/// Confere o programa e preenche a árvore.
pub fn checar(src: &str, prog: &mut Programa) -> Checado {
    let mut s = Sema {
        linhas: Linhas::new(src),
        diags: vec![],
        info: Info::default(),
        globais: HashMap::new(),
        escopos: vec![],
        ctx: Ctx {
            retorno: None,
            em_rotina: false,
            em_etapa: false,
            slots: 0,
            funcao: None,
        },
        chamadas: vec![],
    };
    s.declarar_topo(prog);
    s.chamadas = vec![vec![]; s.info.funcoes.len()];
    let mut idx_funcao = 0u32;
    let mut tem_rotina = false;
    for item in prog.itens.iter_mut() {
        match item {
            Item::Funcao(f) => {
                s.funcao(f, idx_funcao);
                idx_funcao += 1;
            }
            Item::Rotina(r) => {
                tem_rotina = true;
                s.rotina(r);
            }
            _ => {}
        }
    }
    s.recursao();
    if !tem_rotina {
        s.erro(
            "E_SEM_ROTINA",
            Span::new(0, 0),
            "o arquivo precisa de ao menos uma rotina(\"Nome\", horario) { … }".into(),
            None,
        );
    }
    s.diags.sort_by_key(|d| (d.ini, d.fim));
    Checado {
        info: s.info,
        diagnosticos: s.diags,
    }
}
