//! Interpretador (spec/04-execucao.md). Determinístico: relógio virtual, respostas fornecidas
//! por um [`Dispositivo`].

use std::rc::Rc;

use crate::ast::*;
use crate::sema::{Info, ValorGlobal};

/// Dia usado quando o horário da rotina não tem data: 2026-11-05.
pub const DIA_BASE: i64 = dias_civis(2026, 11, 5) * 86400;
pub const LIMITE_ROTINA: i64 = 7200;
pub const LIMITE_PASSOS: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resultado {
    Confirmado,
    Ajuda,
    TempoEsgotado,
}

impl Resultado {
    pub fn nome(self) -> &'static str {
        match self {
            Resultado::Confirmado => "CONFIRMADO",
            Resultado::Ajuda => "AJUDA",
            Resultado::TempoEsgotado => "TEMPO_ESGOTADO",
        }
    }
    pub fn de_nome(s: &str) -> Option<Self> {
        Some(match s {
            "CONFIRMADO" => Resultado::Confirmado,
            "AJUDA" => Resultado::Ajuda,
            "TEMPO_ESGOTADO" => Resultado::TempoEsgotado,
            _ => return None,
        })
    }
}

/// O que o dispositivo responde a uma espera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resposta {
    /// Resultado e segundos até a resposta.
    Dada(Resultado, i64),
    /// Ninguém respondeu.
    Nenhuma,
    /// Ainda não há resposta: interrompe a execução (o chamador pode repetir com mais respostas).
    Pausar,
}

pub trait Dispositivo {
    fn registro(&mut self, linha: &str);
    fn aguardar(&mut self, limite: i64) -> Resposta;
}

/// Respostas de uma lista; quando acaba, devolve [`Resposta::Nenhuma`] ou pausa.
pub struct Roteiro {
    pub respostas: Vec<(Resultado, i64)>,
    pub proxima: usize,
    pub pausar_no_fim: bool,
    pub saida: Vec<String>,
}

impl Roteiro {
    pub fn new(respostas: Vec<(Resultado, i64)>) -> Self {
        Roteiro {
            respostas,
            proxima: 0,
            pausar_no_fim: false,
            saida: vec![],
        }
    }
}

impl Dispositivo for Roteiro {
    fn registro(&mut self, linha: &str) {
        self.saida.push(linha.to_string());
    }
    fn aguardar(&mut self, _limite: i64) -> Resposta {
        match self.respostas.get(self.proxima) {
            Some(&(r, t)) => {
                self.proxima += 1;
                Resposta::Dada(r, t)
            }
            None if self.pausar_no_fim => Resposta::Pausar,
            None => Resposta::Nenhuma,
        }
    }
}

/// Lê respostas como "CONFIRMADO 3, AJUDA" ou uma por linha. Erro: o trecho inválido.
pub fn ler_respostas(s: &str) -> Result<Vec<(Resultado, i64)>, String> {
    let mut v = vec![];
    for parte in s.split([',', '\n', ';']) {
        let parte = parte.trim();
        if parte.is_empty() || parte.starts_with('#') {
            continue;
        }
        let mut it = parte.split_whitespace();
        let nome = it.next().unwrap_or("").to_ascii_uppercase();
        let r = match nome.as_str() {
            "C" => Resultado::Confirmado,
            "A" => Resultado::Ajuda,
            "T" => Resultado::TempoEsgotado,
            n => Resultado::de_nome(n).ok_or_else(|| parte.to_string())?,
        };
        let t = match it.next() {
            Some(x) => x
                .trim_end_matches('s')
                .parse::<i64>()
                .map_err(|_| parte.to_string())?,
            None => 0,
        };
        if it.next().is_some() || t < 0 {
            return Err(parte.to_string());
        }
        v.push((r, t));
    }
    Ok(v)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Valor {
    Num(i32),
    Logico(bool),
    Texto(Rc<str>),
    Duracao(i64),
    Data(Data),
    /// Recurso pelo índice global.
    Recurso(u32),
    Enum(u16, u16),
    Nulo,
    Vazio,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Estado {
    Concluida,
    Cancelada,
    Erro(String),
}

impl Estado {
    pub fn texto(&self) -> String {
        match self {
            Estado::Concluida => "concluida".into(),
            Estado::Cancelada => "cancelada".into(),
            Estado::Erro(m) => format!("erro: {m}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Execucao {
    pub rotinas: Vec<(String, Estado)>,
    /// Se a execução parou esperando uma resposta: o limite da espera.
    pub pausada: Option<i64>,
}

enum Fluxo {
    Retorno(Valor),
    Erro(String),
    Pausa,
}

type R<T> = Result<T, Fluxo>;

fn erro<T>(m: &str) -> R<T> {
    Err(Fluxo::Erro(m.to_string()))
}

struct Maquina<'a, D: Dispositivo> {
    prog: &'a Programa,
    info: &'a Info,
    dev: &'a mut D,
    inicio: i64,
    agora: i64,
    ultima: i64,
    passos: u64,
    abertos: Vec<u32>,
    linha: String,
    pausa_limite: Option<i64>,
}

impl<'a, D: Dispositivo> Maquina<'a, D> {
    fn emite(&mut self, partes: std::fmt::Arguments) {
        use std::fmt::Write;
        self.linha.clear();
        let _ = self.linha.write_fmt(partes);
        let l = std::mem::take(&mut self.linha);
        self.dev.registro(&l);
        self.linha = l;
    }

    fn nome_global(&self, i: u32) -> &'a str {
        &self.info.globais[i as usize].nome
    }

    fn nome_enum(&self, t: u16, v: u16) -> &'a str {
        &self.info.enums[t as usize].valores[v as usize]
    }

    fn avancar(&mut self, s: i64) -> R<()> {
        self.agora += s;
        if self.agora - self.inicio > LIMITE_ROTINA {
            return erro("a rotina excedeu o limite de duas horas");
        }
        Ok(())
    }

    fn passo(&mut self) -> R<()> {
        self.passos += 1;
        if self.passos > LIMITE_PASSOS {
            return erro("possível laço sem fim (limite de passos)");
        }
        Ok(())
    }

    fn fechar(&mut self, motivo: &str) {
        for o in std::mem::take(&mut self.abertos) {
            let n = self.nome_global(o);
            self.emite(format_args!("PARAR_INDICACAO {n} ({motivo})"));
        }
    }

    fn rodar(&mut self) -> Execucao {
        let mut rotinas = vec![];
        for item in &self.prog.itens {
            let Item::Rotina(r) = item else { continue };
            let h = match &r.horario.k {
                ExprK::Data(d) => d.valor(),
                ExprK::Nome {
                    res: Res::Global(g),
                    ..
                } => match self.info.globais[*g as usize].valor {
                    ValorGlobal::Data(d) => Some(d),
                    _ => None,
                },
                _ => None,
            }
            .unwrap_or(Data {
                completa: false,
                seg: 0,
            });
            self.inicio = if h.completa { h.seg } else { DIA_BASE + h.seg };
            self.agora = self.inicio;
            self.ultima = 0;
            self.passos = 0;
            self.abertos.clear();
            self.emite(format_args!("ROTINA {}", r.nome));
            let mut quadro = vec![Valor::Vazio; r.nslots as usize];
            let res = self.bloco(&r.corpo, &mut quadro);
            if matches!(res, Err(Fluxo::Pausa)) {
                return Execucao {
                    rotinas,
                    pausada: self.pausa_limite,
                };
            }
            self.fechar("fim da rotina");
            let estado = match res {
                Ok(()) => Estado::Concluida,
                Err(Fluxo::Retorno(_)) => Estado::Cancelada,
                Err(Fluxo::Erro(m)) => Estado::Erro(m),
                Err(Fluxo::Pausa) => unreachable!(),
            };
            self.emite(format_args!("FIM {}", estado.texto()));
            rotinas.push((r.nome.clone(), estado));
        }
        Execucao {
            rotinas,
            pausada: None,
        }
    }

    fn bloco(&mut self, b: &'a Bloco, q: &mut [Valor]) -> R<()> {
        for s in &b.cmds {
            self.comando(s, q)?;
        }
        Ok(())
    }

    fn comando(&mut self, s: &'a Stmt, q: &mut [Valor]) -> R<()> {
        match &s.k {
            StmtK::Decl { valor, slot, .. } => {
                q[*slot as usize] = self.expr(valor, q)?;
            }
            StmtK::Atrib { valor, res, .. } => {
                let v = self.expr(valor, q)?;
                if let Res::Local(slot) = res {
                    q[*slot as usize] = v;
                }
            }
            StmtK::Expr(e) => {
                self.expr(e, q)?;
            }
            StmtK::Se { ramos, senao } => {
                for (c, b) in ramos {
                    if self.logico(c, q)? {
                        return self.bloco(b, q);
                    }
                }
                if let Some(b) = senao {
                    self.bloco(b, q)?;
                }
            }
            StmtK::Enquanto { cond, corpo } => {
                while self.logico(cond, q)? {
                    self.passo()?;
                    self.bloco(corpo, q)?;
                }
            }
            StmtK::Faca { corpo, cond } => loop {
                self.passo()?;
                self.bloco(corpo, q)?;
                if !self.logico(cond, q)? {
                    break;
                }
            },
            StmtK::Para {
                ini,
                cond,
                upd,
                corpo,
                slot,
                ..
            } => {
                q[*slot as usize] = self.expr(ini, q)?;
                while self.logico(cond, q)? {
                    self.passo()?;
                    self.bloco(corpo, q)?;
                    q[*slot as usize] = self.expr(upd, q)?;
                }
            }
            StmtK::Retorna { valor } => {
                let v = match valor {
                    Some(e) => self.expr(e, q)?,
                    None => Valor::Vazio,
                };
                return Err(Fluxo::Retorno(v));
            }
            StmtK::Etapa { nome, corpo, .. } => {
                self.emite(format_args!("ETAPA {nome}"));
                let r = self.bloco(corpo, q);
                if matches!(r, Err(Fluxo::Pausa)) {
                    return r;
                }
                self.fechar("fim da etapa");
                r?;
            }
        }
        Ok(())
    }

    fn logico(&mut self, e: &'a Expr, q: &mut [Valor]) -> R<bool> {
        match self.expr(e, q)? {
            Valor::Logico(b) => Ok(b),
            _ => erro("condição sem valor lógico"),
        }
    }

    fn expr(&mut self, e: &'a Expr, q: &mut [Valor]) -> R<Valor> {
        Ok(match &e.k {
            ExprK::Num(n) => Valor::Num(*n),
            ExprK::Logico(b) => Valor::Logico(*b),
            ExprK::Texto(s) => Valor::Texto(Rc::from(s.as_str())),
            ExprK::Duracao(s) => Valor::Duracao(*s as i64),
            ExprK::Data(d) => Valor::Data(d.valor().unwrap_or(Data {
                completa: false,
                seg: 0,
            })),
            ExprK::Nulo => Valor::Nulo,
            ExprK::Nome { res, .. } => match *res {
                Res::Local(s) => q[s as usize].clone(),
                Res::Global(g) => match &self.info.globais[g as usize].valor {
                    ValorGlobal::Recurso(_) => Valor::Recurso(g),
                    ValorGlobal::Data(d) => Valor::Data(*d),
                    ValorGlobal::Invalido => Valor::Nulo,
                },
                Res::EnumVal(t, v) => Valor::Enum(t, v),
                _ => Valor::Vazio,
            },
            ExprK::Chamada { args, res, .. } => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.expr(a, q)?);
                }
                match *res {
                    Res::Prim(p) => self.prim(p, vals)?,
                    Res::Funcao(i) => self.chamar(i, vals)?,
                    _ => Valor::Vazio,
                }
            }
            ExprK::Unario { op, e } => match (op, self.expr(e, q)?) {
                (OpUn::Nao, Valor::Logico(b)) => Valor::Logico(!b),
                (OpUn::Menos, Valor::Num(n)) => Valor::Num(
                    n.checked_neg()
                        .ok_or(Fluxo::Erro("estouro de num".into()))?,
                ),
                _ => Valor::Vazio,
            },
            ExprK::Binario { op, l, r, .. } => {
                match op {
                    OpBin::E => return Ok(Valor::Logico(self.logico(l, q)? && self.logico(r, q)?)),
                    OpBin::Ou => {
                        return Ok(Valor::Logico(self.logico(l, q)? || self.logico(r, q)?));
                    }
                    _ => {}
                }
                let a = self.expr(l, q)?;
                let b = self.expr(r, q)?;
                binario(*op, a, b)?
            }
        })
    }

    fn chamar(&mut self, i: u32, args: Vec<Valor>) -> R<Valor> {
        let Item::Funcao(f) = &self.prog.itens[self.info.funcoes[i as usize].item] else {
            return Ok(Valor::Vazio);
        };
        let mut quadro = vec![Valor::Vazio; f.nslots.max(f.params.len() as u32) as usize];
        for (k, v) in args.into_iter().enumerate() {
            quadro[k] = v;
        }
        match self.bloco(&f.corpo, &mut quadro) {
            Ok(()) => Ok(Valor::Vazio),
            Err(Fluxo::Retorno(v)) => Ok(v),
            Err(o) => Err(o),
        }
    }

    fn prim(&mut self, p: Prim, a: Vec<Valor>) -> R<Valor> {
        let arg = a.first().cloned().unwrap_or(Valor::Vazio);
        match (p, arg) {
            (_, Valor::Nulo) => {}
            (Prim::Instruir, Valor::Recurso(m)) => {
                let n = self.nome_global(m);
                self.emite(format_args!("INSTRUIR {n}"));
            }
            (Prim::Indicar, Valor::Recurso(o)) => {
                if !self.abertos.contains(&o) {
                    self.abertos.push(o);
                }
                let n = self.nome_global(o);
                self.emite(format_args!("INDICAR {n}"));
            }
            (Prim::PararIndicacao, Valor::Recurso(o)) => {
                self.abertos.retain(|x| *x != o);
                let n = self.nome_global(o);
                self.emite(format_args!("PARAR_INDICACAO {n}"));
            }
            (Prim::Alertar, Valor::Enum(t, v)) | (Prim::Registrar, Valor::Enum(t, v)) => {
                let n = self.nome_enum(t, v);
                let pn = p.nome();
                self.emite(format_args!("{pn} {n}"));
            }
            (Prim::Esperar, Valor::Duracao(d)) => {
                self.emite(format_args!("ESPERAR {d}s"));
                self.avancar(d)?;
            }
            (Prim::Aguardar, Valor::Duracao(lim)) => {
                let (mut r, mut t) = (Resultado::TempoEsgotado, lim);
                if lim > 0 {
                    match self.dev.aguardar(lim) {
                        Resposta::Pausar => {
                            self.pausa_limite = Some(lim);
                            return Err(Fluxo::Pausa);
                        }
                        Resposta::Dada(rr, tt)
                            if rr != Resultado::TempoEsgotado && (0..=lim).contains(&tt) =>
                        {
                            r = rr;
                            t = tt;
                        }
                        _ => {}
                    }
                }
                self.ultima = t;
                let rn = r.nome();
                self.emite(format_args!("AGUARDAR_CONFIRMACAO {lim}s -> {rn} ({t}s)"));
                self.avancar(t)?;
                let v = match r {
                    Resultado::Confirmado => 0,
                    Resultado::Ajuda => 1,
                    Resultado::TempoEsgotado => 2,
                };
                return Ok(Valor::Enum(RESULTADO, v));
            }
            (Prim::Agora, _) => {
                return Ok(Valor::Data(Data {
                    completa: true,
                    seg: self.agora,
                }));
            }
            (Prim::TempoDeResposta, _) => return Ok(Valor::Duracao(self.ultima)),
            _ => {}
        }
        Ok(Valor::Vazio)
    }
}

fn dur(v: i64) -> R<Valor> {
    if v < 0 {
        erro("duração negativa")
    } else if v > i32::MAX as i64 {
        erro("estouro de duração")
    } else {
        Ok(Valor::Duracao(v))
    }
}

fn num(v: Option<i32>) -> R<Valor> {
    v.map(Valor::Num)
        .ok_or(Fluxo::Erro("estouro de num".into()))
}

fn binario(op: OpBin, a: Valor, b: Valor) -> R<Valor> {
    use OpBin::*;
    use Valor::*;
    Ok(match (op, a, b) {
        (Igual, Data(x), Data(y)) => Logico(x.comparar(y).is_eq()),
        (Diferente, Data(x), Data(y)) => Logico(!x.comparar(y).is_eq()),
        (Igual, x, y) => Logico(x == y),
        (Diferente, x, y) => Logico(x != y),
        (Menor | Maior | MenorIgual | MaiorIgual, x, y) => {
            let o = match (x, y) {
                (Num(x), Num(y)) => x.cmp(&y),
                (Duracao(x), Duracao(y)) => x.cmp(&y),
                (Data(x), Data(y)) => x.comparar(y),
                _ => return erro("comparação inválida"),
            };
            Logico(match op {
                Menor => o.is_lt(),
                Maior => o.is_gt(),
                MenorIgual => o.is_le(),
                _ => o.is_ge(),
            })
        }
        (Soma, Num(x), Num(y)) => num(x.checked_add(y))?,
        (Sub, Num(x), Num(y)) => num(x.checked_sub(y))?,
        (Mul, Num(x), Num(y)) => num(x.checked_mul(y))?,
        (Div | Resto, Num(_), Num(0)) => return erro("divisão por zero"),
        (Div, Num(x), Num(y)) => num(x.checked_div(y))?,
        (Resto, Num(x), Num(y)) => Num(if y == -1 { 0 } else { x % y }),
        (Soma, Duracao(x), Duracao(y)) => dur(x + y)?,
        (Sub, Duracao(x), Duracao(y)) => dur(x - y)?,
        (Mul, Duracao(x), Num(y)) | (Mul, Num(y), Duracao(x)) => dur(x * y as i64)?,
        (Div, Duracao(_), Num(0)) => return erro("divisão por zero"),
        (Div, Duracao(x), Num(y)) => dur(x / y as i64)?,
        _ => return erro("operação inválida"),
    })
}

/// Executa todas as rotinas do programa (que precisa estar sem erros).
pub fn executar<D: Dispositivo>(prog: &Programa, info: &Info, dev: &mut D) -> Execucao {
    let mut m = Maquina {
        prog,
        info,
        dev,
        inicio: 0,
        agora: 0,
        ultima: 0,
        passos: 0,
        abertos: vec![],
        linha: String::new(),
        pausa_limite: None,
    };
    m.rodar()
}
