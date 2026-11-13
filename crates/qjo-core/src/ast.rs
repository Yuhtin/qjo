//! Árvore sintática. Os campos `tipo`, `res` e `slot` são preenchidos pela semântica.

/// Trecho em bytes no fonte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub ini: u32,
    pub fim: u32,
}

impl Span {
    pub fn new(ini: u32, fim: u32) -> Self {
        Span { ini, fim }
    }
    pub fn juntar(self, o: Span) -> Span {
        Span {
            ini: self.ini.min(o.ini),
            fim: self.fim.max(o.fim),
        }
    }
    pub fn contem(self, off: u32) -> bool {
        self.ini <= off && off <= self.fim
    }
}

pub const RESULTADO: u16 = 0;
pub const INTENSIDADE: u16 = 1;
pub const EVENTO: u16 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tipo {
    Num,
    Logico,
    Texto,
    Duracao,
    Data,
    Objeto,
    Mensagem,
    /// Enum pelo índice: 0 resultado, 1 intensidade, 2 evento, 3+ declarados.
    Enum(u16),
    /// Sem valor (função sem tipo).
    Vazio,
    /// O literal `nulo`.
    Nulo,
    /// Erro já relatado; não gera novos erros.
    #[default]
    Erro,
}

impl Tipo {
    pub fn aceita_nulo(self) -> bool {
        matches!(self, Tipo::Objeto | Tipo::Mensagem | Tipo::Enum(RESULTADO))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prim {
    Instruir,
    Indicar,
    PararIndicacao,
    Aguardar,
    Alertar,
    Esperar,
    Registrar,
    Agora,
    TempoDeResposta,
}

pub const PRIMITIVAS: [(&str, Prim); 9] = [
    ("INSTRUIR", Prim::Instruir),
    ("INDICAR", Prim::Indicar),
    ("PARAR_INDICACAO", Prim::PararIndicacao),
    ("AGUARDAR_CONFIRMACAO", Prim::Aguardar),
    ("ALERTAR", Prim::Alertar),
    ("ESPERAR", Prim::Esperar),
    ("REGISTRAR", Prim::Registrar),
    ("AGORA", Prim::Agora),
    ("TEMPO_DE_RESPOSTA", Prim::TempoDeResposta),
];

impl Prim {
    pub fn nome(self) -> &'static str {
        PRIMITIVAS
            .iter()
            .find(|(_, p)| *p == self)
            .map(|(n, _)| *n)
            .unwrap()
    }
    pub fn params(self) -> &'static [Tipo] {
        match self {
            Prim::Instruir => &[Tipo::Mensagem],
            Prim::Indicar | Prim::PararIndicacao => &[Tipo::Objeto],
            Prim::Aguardar | Prim::Esperar => &[Tipo::Duracao],
            Prim::Alertar => &[Tipo::Enum(INTENSIDADE)],
            Prim::Registrar => &[Tipo::Enum(EVENTO)],
            Prim::Agora | Prim::TempoDeResposta => &[],
        }
    }
    pub fn retorno(self) -> Tipo {
        match self {
            Prim::Aguardar => Tipo::Enum(RESULTADO),
            Prim::Agora => Tipo::Data,
            Prim::TempoDeResposta => Tipo::Duracao,
            _ => Tipo::Vazio,
        }
    }
    pub fn descricao(self) -> &'static str {
        match self {
            Prim::Instruir => "Apresenta a orientação associada à mensagem.",
            Prim::Indicar => {
                "Liga a indicação do objeto até PARAR_INDICACAO, o fim da etapa ou da rotina."
            }
            Prim::PararIndicacao => "Desliga a indicação do objeto, se estiver ligada.",
            Prim::Aguardar => {
                "Espera uma resposta até o limite e devolve CONFIRMADO, AJUDA ou TEMPO_ESGOTADO."
            }
            Prim::Alertar => "Chama a atenção no nível pedido.",
            Prim::Esperar => "Pausa a rotina pela duração, sem esperar resposta.",
            Prim::Registrar => "Registra um evento para o terapeuta consultar.",
            Prim::Agora => "A data e a hora atuais.",
            Prim::TempoDeResposta => "Quanto durou a última espera por confirmação.",
        }
    }
}

/// A que um nome se refere, depois da semântica.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Res {
    #[default]
    Nenhum,
    Local(u32),
    Global(u32),
    EnumVal(u16, u16),
    Funcao(u32),
    Prim(Prim),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpUn {
    Nao,
    Menos,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpBin {
    Ou,
    E,
    Igual,
    Diferente,
    Menor,
    Maior,
    MenorIgual,
    MaiorIgual,
    Soma,
    Sub,
    Mul,
    Div,
    Resto,
}

impl OpBin {
    pub fn simbolo(self) -> &'static str {
        use OpBin::*;
        match self {
            Ou => "||",
            E => "&&",
            Igual => "==",
            Diferente => "!=",
            Menor => "<",
            Maior => ">",
            MenorIgual => "<=",
            MaiorIgual => ">=",
            Soma => "+",
            Sub => "-",
            Mul => "*",
            Div => "/",
            Resto => "%",
        }
    }
}

/// Literal `data"..."` como escrito (a validade é semântica).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataLit {
    pub completa: bool,
    pub ano: i32,
    pub mes: u32,
    pub dia: u32,
    pub h: u32,
    pub m: u32,
    pub s: u32,
}

/// Valor de data: segundos desde 1970-01-01 (completa) ou segundos do dia (só horário).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Data {
    pub completa: bool,
    pub seg: i64,
}

impl Data {
    pub fn hora_do_dia(self) -> i64 {
        if self.completa {
            self.seg.rem_euclid(86400)
        } else {
            self.seg
        }
    }
    pub fn comparar(self, o: Data) -> std::cmp::Ordering {
        if self.completa && o.completa {
            self.seg.cmp(&o.seg)
        } else {
            self.hora_do_dia().cmp(&o.hora_do_dia())
        }
    }
}

/// Dias desde 1970-01-01 (algoritmo de Howard Hinnant).
pub const fn dias_civis(ano: i32, mes: u32, dia: u32) -> i64 {
    let y = ano as i64 - if mes <= 2 { 1 } else { 0 };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let m = mes as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + dia as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

impl DataLit {
    pub fn valor(self) -> Option<Data> {
        if self.h > 23 || self.m > 59 || self.s > 59 {
            return None;
        }
        let tod = (self.h * 3600 + self.m * 60 + self.s) as i64;
        if !self.completa {
            return Some(Data {
                completa: false,
                seg: tod,
            });
        }
        let bissexto = (self.ano % 4 == 0 && self.ano % 100 != 0) || self.ano % 400 == 0;
        let dias_mes = match self.mes {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if bissexto => 29,
            2 => 28,
            _ => return None,
        };
        if self.dia == 0 || self.dia > dias_mes {
            return None;
        }
        Some(Data {
            completa: true,
            seg: dias_civis(self.ano, self.mes, self.dia) * 86400 + tod,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub k: ExprK,
    pub span: Span,
    pub tipo: Tipo,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprK {
    Num(i32),
    Logico(bool),
    Texto(String),
    /// Segundos, possivelmente acima do máximo (a semântica confere).
    Duracao(u64),
    Data(DataLit),
    Nulo,
    Nome {
        nome: String,
        res: Res,
    },
    Chamada {
        nome: String,
        nome_span: Span,
        args: Vec<Expr>,
        res: Res,
    },
    Unario {
        op: OpUn,
        e: Box<Expr>,
    },
    Binario {
        op: OpBin,
        l: Box<Expr>,
        r: Box<Expr>,
        op_span: Span,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TipoSint {
    pub nome: String,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bloco {
    pub cmds: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub k: StmtK,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)] // árvore pequena; caixas só complicariam
pub enum StmtK {
    Decl {
        tipo: TipoSint,
        nome: String,
        nome_span: Span,
        valor: Expr,
        slot: u32,
        t: Tipo,
    },
    Atrib {
        nome: String,
        nome_span: Span,
        valor: Expr,
        res: Res,
    },
    Expr(Expr),
    Se {
        ramos: Vec<(Expr, Bloco)>,
        senao: Option<Bloco>,
    },
    Enquanto {
        cond: Expr,
        corpo: Bloco,
    },
    Faca {
        corpo: Bloco,
        cond: Expr,
    },
    Para {
        tipo: TipoSint,
        var: String,
        var_span: Span,
        ini: Expr,
        cond: Expr,
        alvo: String,
        alvo_span: Span,
        upd: Expr,
        corpo: Bloco,
        slot: u32,
    },
    Retorna {
        valor: Option<Expr>,
    },
    Etapa {
        nome: String,
        nome_span: Span,
        corpo: Bloco,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub tipo: TipoSint,
    pub nome: String,
    pub nome_span: Span,
    pub t: Tipo,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Funcao {
    pub nome: String,
    pub nome_span: Span,
    pub ret: Option<TipoSint>,
    pub ret_t: Tipo,
    pub params: Vec<Param>,
    pub corpo: Bloco,
    pub nslots: u32,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rotina {
    pub nome: String,
    pub nome_span: Span,
    pub horario: Expr,
    pub corpo: Bloco,
    pub nslots: u32,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Global {
        tipo: TipoSint,
        nome: String,
        nome_span: Span,
        valor: Expr,
        span: Span,
    },
    Enum {
        nome: String,
        nome_span: Span,
        valores: Vec<(String, Span)>,
        span: Span,
    },
    Funcao(Funcao),
    Rotina(Rotina),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Programa {
    pub itens: Vec<Item>,
}
