//! Análise sintática descendente recursiva, com recuperação (spec/02-sintaxe.md).

use crate::ast::*;
use crate::diag::{Diagnostico, Fase, Linhas, Sugestao};
use crate::lexer::{Tok, Token, decodificar_texto, duracao_segundos};

pub struct Analisado {
    pub programa: Programa,
    pub diagnosticos: Vec<Diagnostico>,
}

struct Parser<'a> {
    src: &'a str,
    t: Vec<Token>,
    p: usize,
    diags: Vec<Diagnostico>,
    ultimo_erro: Option<usize>,
    linhas: Linhas<'a>,
}

type R<T> = Result<T, ()>;

fn nome_tok(t: Tok) -> &'static str {
    match t {
        Tok::AbrePar => "\"(\"",
        Tok::FechaPar => "\")\"",
        Tok::AbreChave => "\"{\"",
        Tok::FechaChave => "\"}\"",
        Tok::PontoVirgula => "\";\"",
        Tok::Sep => "\",\"",
        Tok::Atribuicao => "\"=\"",
        Tok::Id => "um nome",
        Tok::Texto => "um texto entre aspas",
        Tok::Enquanto => "\"enquanto\"",
        _ => t.nome(),
    }
}

impl<'a> Parser<'a> {
    fn cur(&self) -> Option<Token> {
        self.t.get(self.p).copied()
    }
    fn olha(&self, k: usize) -> Option<Tok> {
        self.t.get(self.p + k).map(|t| t.tipo)
    }
    fn em(&self, t: Tok) -> bool {
        self.olha(0) == Some(t)
    }
    fn lex(&self, t: Token) -> &'a str {
        t.lexema(self.src)
    }
    fn avanca(&mut self) -> Token {
        let t = self.t[self.p];
        self.p += 1;
        t
    }
    fn span_de(&self, ini: Token) -> Span {
        let fim = if self.p > 0 {
            self.t[self.p - 1].fim
        } else {
            ini.fim
        };
        Span::new(ini.ini, fim.max(ini.fim))
    }

    fn descr(&self, t: Option<Token>) -> String {
        match t {
            None => "o fim do arquivo".into(),
            Some(t) => match t.tipo {
                Tok::Id => format!("o nome \"{}\"", self.lex(t)),
                Tok::Texto => format!("o texto {}", self.lex(t)),
                Tok::Inteiro | Tok::Duracao | Tok::Data => format!("o valor {}", self.lex(t)),
                _ => format!("\"{}\"", self.lex(t)),
            },
        }
    }

    fn erro_aqui(&mut self, msg: String, sugestao: Option<Sugestao>) {
        if self.ultimo_erro == Some(self.p) {
            return;
        }
        self.ultimo_erro = Some(self.p);
        let (ini, fim) = match self.cur() {
            Some(t) => (t.ini as usize, t.fim as usize),
            None => (self.src.len(), self.src.len()),
        };
        let (linha, coluna) = self.linhas.posicao(ini);
        self.diags.push(Diagnostico {
            codigo: "E_SINT",
            mensagem: msg,
            ini,
            fim,
            linha,
            coluna,
            fase: Fase::Sintaxe,
            sugestao,
        });
    }

    /// Erro de algo que falta logo depois do token anterior (";" ou ")").
    fn erro_falta_depois(&mut self, falta: &str, msg: String) {
        if self.ultimo_erro == Some(self.p) {
            return;
        }
        self.ultimo_erro = Some(self.p);
        let ant = self.t[self.p - 1];
        let pos = ant.fim as usize;
        let (linha, coluna) = self.linhas.posicao(pos);
        let sugestao = Sugestao::troca(
            format!("Inserir \"{falta}\" depois de \"{}\"", self.lex(ant)),
            pos,
            pos,
            falta,
        );
        self.diags.push(Diagnostico {
            codigo: "E_SINT",
            mensagem: msg,
            ini: pos,
            fim: pos,
            linha,
            coluna,
            fase: Fase::Sintaxe,
            sugestao: Some(sugestao),
        });
    }

    fn espera(&mut self, t: Tok, onde: &str) -> R<Token> {
        if self.em(t) {
            return Ok(self.avanca());
        }
        let msg = format!(
            "esperava {}{}{}, mas veio {}",
            nome_tok(t),
            if onde.is_empty() { "" } else { " " },
            onde,
            self.descr(self.cur())
        );
        match t {
            Tok::PontoVirgula | Tok::FechaPar if self.p > 0 => {
                let falta = if t == Tok::PontoVirgula { ";" } else { ")" };
                self.erro_falta_depois(falta, msg)
            }
            Tok::FechaChave if self.cur().is_none() => {
                let fim = self.src.len();
                self.erro_aqui(
                    msg,
                    Some(Sugestao::troca("Fechar a chave no fim", fim, fim, "\n}")),
                )
            }
            _ => self.erro_aqui(msg, None),
        }
        Err(())
    }

    fn primeiro_da_linha(&self) -> bool {
        self.p == 0 || self.p >= self.t.len() || self.t[self.p].linha > self.t[self.p - 1].linha
    }

    // ---------- topo ----------

    fn programa(&mut self) -> Programa {
        let mut itens = vec![];
        while self.p < self.t.len() {
            let ini = self.p;
            match self.item() {
                Ok(i) => itens.push(i),
                Err(()) => {
                    if self.p == ini {
                        self.p += 1;
                    }
                    self.sincroniza_topo();
                }
            }
        }
        Programa { itens }
    }

    fn sincroniza_topo(&mut self) {
        while let Some(t) = self.cur() {
            if self.primeiro_da_linha() && matches!(t.tipo, Tok::Rotina | Tok::Funcao | Tok::Tipo) {
                return;
            }
            if t.tipo == Tok::AbreChave {
                self.pula_bloco();
            } else {
                self.p += 1;
            }
        }
    }

    fn eh_tipo_aqui(&self) -> bool {
        match self.olha(0) {
            Some(Tok::Tipo) => true,
            Some(Tok::Id) => self.olha(1) == Some(Tok::Id),
            _ => false,
        }
    }

    fn tipo(&mut self) -> TipoSint {
        let t = self.avanca();
        TipoSint {
            nome: self.lex(t).to_string(),
            span: Span::new(t.ini, t.fim),
        }
    }

    fn item(&mut self) -> R<Item> {
        let ini = self.cur().unwrap();
        match ini.tipo {
            Tok::Funcao => self.funcao().map(Item::Funcao),
            Tok::Rotina => self.rotina().map(Item::Rotina),
            Tok::Tipo if self.lex(ini) == "enum" && self.olha(1) == Some(Tok::Id) => {
                self.enum_decl()
            }
            _ if self.eh_tipo_aqui() => {
                let tipo = self.tipo();
                let nome = self.espera(Tok::Id, "com o nome da declaração")?;
                self.espera(Tok::Atribuicao, "depois do nome")?;
                let valor = self.expr()?;
                self.espera(Tok::PontoVirgula, "no fim da declaração")?;
                Ok(Item::Global {
                    tipo,
                    nome: self.lex(nome).to_string(),
                    nome_span: Span::new(nome.ini, nome.fim),
                    valor,
                    span: self.span_de(ini),
                })
            }
            _ => {
                let msg = format!(
                    "no topo do arquivo só cabem rotina, funcao, enum e declarações de recursos e horários; veio {}",
                    self.descr(Some(ini))
                );
                self.erro_aqui(msg, None);
                Err(())
            }
        }
    }

    fn enum_decl(&mut self) -> R<Item> {
        let ini = self.avanca();
        let nome = self.avanca();
        self.espera(Tok::AbreChave, "abrindo os valores do enum")?;
        let mut valores = vec![];
        loop {
            let v = self.espera(Tok::Id, "com um valor do enum")?;
            valores.push((self.lex(v).to_string(), Span::new(v.ini, v.fim)));
            if !self.em(Tok::Sep) {
                break;
            }
            self.avanca();
        }
        self.espera(Tok::FechaChave, "fechando o enum")?;
        if self.em(Tok::PontoVirgula) {
            self.avanca();
        }
        Ok(Item::Enum {
            nome: self.lex(nome).to_string(),
            nome_span: Span::new(nome.ini, nome.fim),
            valores,
            span: self.span_de(ini),
        })
    }

    fn funcao(&mut self) -> R<Funcao> {
        let ini = self.avanca();
        let ret = if self.eh_tipo_aqui() {
            Some(self.tipo())
        } else {
            None
        };
        let nome = self.espera(Tok::Id, "com o nome da função")?;
        self.espera(Tok::AbrePar, "depois do nome da função")?;
        let mut params = vec![];
        if !self.em(Tok::FechaPar) {
            loop {
                if !matches!(self.olha(0), Some(Tok::Tipo | Tok::Id)) {
                    let msg = format!(
                        "esperava o tipo do parâmetro, mas veio {}",
                        self.descr(self.cur())
                    );
                    self.erro_aqui(msg, None);
                    return Err(());
                }
                let tipo = self.tipo();
                let n = self.espera(Tok::Id, "com o nome do parâmetro")?;
                params.push(Param {
                    tipo,
                    nome: self.lex(n).to_string(),
                    nome_span: Span::new(n.ini, n.fim),
                    t: Tipo::Erro,
                });
                if !self.em(Tok::Sep) {
                    break;
                }
                self.avanca();
            }
        }
        self.espera(Tok::FechaPar, "depois dos parâmetros")?;
        let corpo = self.bloco()?;
        Ok(Funcao {
            nome: self.lex(nome).to_string(),
            nome_span: Span::new(nome.ini, nome.fim),
            ret,
            ret_t: Tipo::Erro,
            params,
            corpo,
            nslots: 0,
            span: self.span_de(ini),
        })
    }

    fn texto_nome(&mut self, onde: &str) -> R<(String, Span)> {
        if self.em(Tok::Texto) {
            let t = self.avanca();
            return Ok((decodificar_texto(self.lex(t)), Span::new(t.ini, t.fim)));
        }
        let msg = format!(
            "esperava o nome {onde} entre aspas, mas veio {}",
            self.descr(self.cur())
        );
        self.erro_aqui(msg, None);
        Err(())
    }

    fn rotina(&mut self) -> R<Rotina> {
        let ini = self.avanca();
        self.espera(Tok::AbrePar, "depois de \"rotina\"")?;
        let (nome, nome_span) = self.texto_nome("da rotina")?;
        self.espera(Tok::Sep, "entre o nome e o horário da rotina")?;
        let horario = self.expr()?;
        self.espera(Tok::FechaPar, "depois do horário da rotina")?;
        let corpo = self.bloco()?;
        Ok(Rotina {
            nome,
            nome_span,
            horario,
            corpo,
            nslots: 0,
            span: self.span_de(ini),
        })
    }

    // ---------- comandos ----------

    fn bloco(&mut self) -> R<Bloco> {
        let abre = self.espera(Tok::AbreChave, "abrindo o bloco")?;
        let mut cmds = vec![];
        loop {
            match self.olha(0) {
                Some(Tok::FechaChave) => break,
                None => {
                    self.espera(
                        Tok::FechaChave,
                        &format!("fechando o bloco aberto na linha {}", abre.linha),
                    )?;
                }
                _ => {}
            }
            let ini = self.p;
            match self.comando() {
                Ok(s) => cmds.push(s),
                Err(()) => {
                    if self.p == ini {
                        self.p += 1;
                    }
                    self.sincroniza_comando(ini);
                }
            }
        }
        self.avanca();
        Ok(Bloco {
            cmds,
            span: self.span_de(abre),
        })
    }

    /// Descarta até um ponto seguro. `ini` é onde o comando começou: parênteses abertos desde
    /// lá ainda precisam fechar antes de um ";" valer como fim, e um bloco "{…}" do comando
    /// quebrado é pulado inteiro (senão o "}" dele fecharia o bloco de fora).
    fn sincroniza_comando(&mut self, ini: usize) {
        let mut parens: i32 = self.t[ini..self.p.min(self.t.len())]
            .iter()
            .map(|t| match t.tipo {
                Tok::AbrePar => 1,
                Tok::FechaPar => -1,
                _ => 0,
            })
            .sum();
        while let Some(t) = self.cur() {
            match t.tipo {
                Tok::AbrePar => parens += 1,
                Tok::FechaPar => parens -= 1,
                Tok::PontoVirgula if parens <= 0 => {
                    self.p += 1;
                    return;
                }
                Tok::AbreChave if parens <= 0 => {
                    self.pula_bloco();
                    return;
                }
                Tok::FechaChave => return,
                Tok::Se | Tok::Enquanto | Tok::Faca | Tok::Para | Tok::Retorna | Tok::Etapa
                    if parens <= 0 =>
                {
                    return;
                }
                Tok::Tipo | Tok::Id if parens <= 0 && self.primeiro_da_linha() => return,
                _ => {}
            }
            self.p += 1;
        }
    }

    /// Pula um "{…}" balanceado a partir do "{" atual.
    fn pula_bloco(&mut self) {
        let mut nivel = 0;
        while let Some(t) = self.cur() {
            self.p += 1;
            match t.tipo {
                Tok::AbreChave => nivel += 1,
                Tok::FechaChave => {
                    nivel -= 1;
                    if nivel == 0 {
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    fn comando(&mut self) -> R<Stmt> {
        let ini = self.cur().unwrap();
        let k = match ini.tipo {
            Tok::Se => self.se()?,
            Tok::Enquanto => {
                self.avanca();
                let cond = self.condicao("enquanto")?;
                let corpo = self.bloco()?;
                StmtK::Enquanto { cond, corpo }
            }
            Tok::Faca => {
                self.avanca();
                let corpo = self.bloco()?;
                self.espera(Tok::Enquanto, "depois do bloco do \"faca\"")?;
                let cond = self.condicao("enquanto")?;
                self.espera(Tok::PontoVirgula, "depois do \"faca … enquanto (…)\"")?;
                StmtK::Faca { corpo, cond }
            }
            Tok::Para => self.para()?,
            Tok::Retorna => {
                self.avanca();
                let valor = if self.em(Tok::PontoVirgula) {
                    None
                } else {
                    Some(self.expr()?)
                };
                self.espera(Tok::PontoVirgula, "depois do \"retorna\"")?;
                StmtK::Retorna { valor }
            }
            Tok::Etapa => {
                self.avanca();
                self.espera(Tok::AbrePar, "depois de \"etapa\"")?;
                let (nome, nome_span) = self.texto_nome("da etapa")?;
                self.espera(Tok::FechaPar, "depois do nome da etapa")?;
                let corpo = self.bloco()?;
                StmtK::Etapa {
                    nome,
                    nome_span,
                    corpo,
                }
            }
            _ if self.eh_tipo_aqui() => {
                let tipo = self.tipo();
                let nome = self.espera(Tok::Id, "com o nome da variável")?;
                self.espera(
                    Tok::Atribuicao,
                    "depois do nome (toda variável começa com um valor)",
                )?;
                let valor = self.expr()?;
                self.espera(Tok::PontoVirgula, "no fim da declaração")?;
                StmtK::Decl {
                    tipo,
                    nome: self.lex(nome).to_string(),
                    nome_span: Span::new(nome.ini, nome.fim),
                    valor,
                    slot: 0,
                    t: Tipo::Erro,
                }
            }
            Tok::Id if self.olha(1) == Some(Tok::Atribuicao) => {
                let nome = self.avanca();
                self.avanca();
                let valor = self.expr()?;
                self.espera(Tok::PontoVirgula, "no fim da atribuição")?;
                StmtK::Atrib {
                    nome: self.lex(nome).to_string(),
                    nome_span: Span::new(nome.ini, nome.fim),
                    valor,
                    res: Res::Nenhum,
                }
            }
            _ => {
                let e = self.expr()?;
                self.espera(Tok::PontoVirgula, "no fim do comando")?;
                StmtK::Expr(e)
            }
        };
        Ok(Stmt {
            k,
            span: self.span_de(ini),
        })
    }

    fn condicao(&mut self, depois: &str) -> R<Expr> {
        self.espera(Tok::AbrePar, &format!("depois de \"{depois}\""))?;
        let e = self.expr()?;
        self.espera(Tok::FechaPar, "fechando a condição")?;
        Ok(e)
    }

    fn se(&mut self) -> R<StmtK> {
        self.avanca();
        let mut ramos = vec![];
        let c = self.condicao("q?")?;
        let b = self.bloco()?;
        ramos.push((c, b));
        let mut senao = None;
        loop {
            match self.olha(0) {
                Some(Tok::SenaoSe) => {
                    self.avanca();
                    let c = self.condicao("qjo?")?;
                    let b = self.bloco()?;
                    ramos.push((c, b));
                }
                Some(Tok::Senao) => {
                    self.avanca();
                    senao = Some(self.bloco()?);
                    break;
                }
                _ => break,
            }
        }
        Ok(StmtK::Se { ramos, senao })
    }

    fn para(&mut self) -> R<StmtK> {
        self.avanca();
        self.espera(Tok::AbrePar, "depois de \"para\"")?;
        if !matches!(self.olha(0), Some(Tok::Tipo | Tok::Id)) {
            let msg = format!(
                "esperava a declaração da variável do laço (num i = 0), mas veio {}",
                self.descr(self.cur())
            );
            self.erro_aqui(msg, None);
            return Err(());
        }
        if self.olha(0) == Some(Tok::Id) && self.olha(1) == Some(Tok::Atribuicao) {
            let t = self.cur().unwrap();
            let msg = format!(
                "faltou o tipo da variável do laço: esperava num {}",
                self.lex(t)
            );
            self.erro_aqui(
                msg,
                Some(Sugestao::troca(
                    "Inserir num",
                    t.ini as usize,
                    t.ini as usize,
                    "num ",
                )),
            );
            return Err(());
        }
        let tipo = self.tipo();
        let var = self.espera(Tok::Id, "com o nome da variável do laço")?;
        self.espera(Tok::Atribuicao, "no início do \"para\"")?;
        let ini = self.expr()?;
        self.espera(Tok::PontoVirgula, "depois do valor inicial do \"para\"")?;
        let cond = self.expr()?;
        self.espera(Tok::PontoVirgula, "depois da condição do \"para\"")?;
        let alvo = self.espera(Tok::Id, "com a variável do laço na atualização")?;
        self.espera(Tok::Atribuicao, "na atualização do \"para\"")?;
        let upd = self.expr()?;
        self.espera(Tok::FechaPar, "fechando o cabeçalho do \"para\"")?;
        let corpo = self.bloco()?;
        Ok(StmtK::Para {
            tipo,
            var: self.lex(var).to_string(),
            var_span: Span::new(var.ini, var.fim),
            ini,
            cond,
            alvo: self.lex(alvo).to_string(),
            alvo_span: Span::new(alvo.ini, alvo.fim),
            upd,
            corpo,
            slot: 0,
        })
    }

    // ---------- expressões ----------

    fn expr(&mut self) -> R<Expr> {
        self.binario(0)
    }

    fn op_bin(&self, nivel: u8) -> Option<OpBin> {
        let t = self.cur()?;
        let l = self.lex(t);
        let op = match (t.tipo, l) {
            (Tok::Ou, _) => OpBin::Ou,
            (Tok::E, _) => OpBin::E,
            (Tok::Igual, _) => OpBin::Igual,
            (Tok::Diferente, _) => OpBin::Diferente,
            (Tok::OpRel, "<") => OpBin::Menor,
            (Tok::OpRel, _) => OpBin::Maior,
            (Tok::OpRelIgual, "<=") => OpBin::MenorIgual,
            (Tok::OpRelIgual, _) => OpBin::MaiorIgual,
            (Tok::OpSoma, "+") => OpBin::Soma,
            (Tok::OpSoma, _) => OpBin::Sub,
            (Tok::OpMult, "*") => OpBin::Mul,
            (Tok::OpMult, "/") => OpBin::Div,
            (Tok::OpMult, _) => OpBin::Resto,
            _ => return None,
        };
        let n = match op {
            OpBin::Ou => 0,
            OpBin::E => 1,
            OpBin::Igual | OpBin::Diferente => 2,
            OpBin::Menor | OpBin::Maior | OpBin::MenorIgual | OpBin::MaiorIgual => 3,
            OpBin::Soma | OpBin::Sub => 4,
            _ => 5,
        };
        (n == nivel).then_some(op)
    }

    fn binario(&mut self, nivel: u8) -> R<Expr> {
        if nivel > 5 {
            return self.unario();
        }
        let mut l = self.binario(nivel + 1)?;
        while let Some(op) = self.op_bin(nivel) {
            let ot = self.avanca();
            let r = self.binario(nivel + 1)?;
            let span = l.span.juntar(r.span);
            let op_span = Span::new(ot.ini, ot.fim);
            l = Expr {
                k: ExprK::Binario {
                    op,
                    l: Box::new(l),
                    r: Box::new(r),
                    op_span,
                },
                span,
                tipo: Tipo::Erro,
            };
        }
        Ok(l)
    }

    fn unario(&mut self) -> R<Expr> {
        let op = match self.cur() {
            Some(t) if t.tipo == Tok::Nao => Some(OpUn::Nao),
            Some(t) if t.tipo == Tok::OpSoma && self.lex(t) == "-" => Some(OpUn::Menos),
            _ => None,
        };
        if let Some(op) = op {
            let t = self.avanca();
            let e = self.unario()?;
            let span = Span::new(t.ini, e.span.fim);
            return Ok(Expr {
                k: ExprK::Unario { op, e: Box::new(e) },
                span,
                tipo: Tipo::Erro,
            });
        }
        self.primario()
    }

    fn primario(&mut self) -> R<Expr> {
        let Some(t) = self.cur() else {
            let msg =
                "esperava um valor, um nome ou uma chamada, mas veio o fim do arquivo".to_string();
            self.erro_aqui(msg, None);
            return Err(());
        };
        let l = self.lex(t);
        let span = Span::new(t.ini, t.fim);
        let k = match t.tipo {
            Tok::Inteiro => ExprK::Num(l.parse::<i64>().unwrap_or(i64::MAX) as i32),
            Tok::Duracao => ExprK::Duracao(duracao_segundos(l).unwrap_or(u64::MAX)),
            Tok::Data => ExprK::Data(ler_data(l)),
            Tok::Texto => ExprK::Texto(decodificar_texto(l)),
            Tok::Logico => ExprK::Logico(l == "verdadeiro"),
            Tok::Nulo => ExprK::Nulo,
            Tok::Id => {
                self.avanca();
                if !self.em(Tok::AbrePar) {
                    return Ok(Expr {
                        k: ExprK::Nome {
                            nome: l.to_string(),
                            res: Res::Nenhum,
                        },
                        span,
                        tipo: Tipo::Erro,
                    });
                }
                self.avanca();
                let mut args = vec![];
                if !self.em(Tok::FechaPar) {
                    loop {
                        args.push(self.expr()?);
                        if !self.em(Tok::Sep) {
                            break;
                        }
                        self.avanca();
                    }
                }
                self.espera(Tok::FechaPar, &format!("fechando a chamada de {l}"))?;
                let span = self.span_de(t);
                return Ok(Expr {
                    k: ExprK::Chamada {
                        nome: l.to_string(),
                        nome_span: Span::new(t.ini, t.fim),
                        args,
                        res: Res::Nenhum,
                    },
                    span,
                    tipo: Tipo::Erro,
                });
            }
            Tok::AbrePar => {
                self.avanca();
                let mut e = self.expr()?;
                self.espera(Tok::FechaPar, "fechando o parêntese")?;
                e.span = self.span_de(t);
                return Ok(e);
            }
            _ => {
                let msg = format!(
                    "esperava um valor, um nome ou uma chamada, mas veio {}",
                    self.descr(Some(t))
                );
                self.erro_aqui(msg, None);
                return Err(());
            }
        };
        self.avanca();
        Ok(Expr {
            k,
            span,
            tipo: Tipo::Erro,
        })
    }
}

fn ler_data(l: &str) -> DataLit {
    let c = &l[5..l.len() - 1];
    let n = |r: std::ops::Range<usize>| c[r].parse::<u32>().unwrap_or(0);
    if c.len() == 8 {
        DataLit {
            completa: false,
            ano: 0,
            mes: 0,
            dia: 0,
            h: n(0..2),
            m: n(3..5),
            s: n(6..8),
        }
    } else {
        DataLit {
            completa: true,
            ano: n(0..4) as i32,
            mes: n(5..7),
            dia: n(8..10),
            h: n(11..13),
            m: n(14..16),
            s: n(17..19),
        }
    }
}

/// Analisa os tokens (os comentários são ignorados).
pub fn parse(src: &str, tokens: &[Token]) -> Analisado {
    let t: Vec<Token> = tokens
        .iter()
        .copied()
        .filter(|t| t.tipo != Tok::Coment)
        .collect();
    let mut p = Parser {
        src,
        t,
        p: 0,
        diags: vec![],
        ultimo_erro: None,
        linhas: Linhas::new(src),
    };
    let programa = p.programa();
    Analisado {
        programa,
        diagnosticos: p.diags,
    }
}
