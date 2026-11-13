//! Análise léxica (spec/01-lexico.md).

use crate::diag::{Diagnostico, Fase, Sugestao};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tok {
    Se,
    SenaoSe,
    Senao,
    Para,
    Enquanto,
    Faca,
    Retorna,
    Rotina,
    Etapa,
    Funcao,
    Logico,
    Nulo,
    Tipo,
    OpSoma,
    OpMult,
    OpRel,
    OpRelIgual,
    Igual,
    Diferente,
    E,
    Ou,
    Nao,
    Atribuicao,
    AbrePar,
    FechaPar,
    AbreChave,
    FechaChave,
    Sep,
    PontoVirgula,
    Id,
    Inteiro,
    Texto,
    Duracao,
    Data,
    Coment,
}

impl Tok {
    /// Nome do token como na tabela da spec.
    pub fn nome(self) -> &'static str {
        use Tok::*;
        match self {
            Se => "SE",
            SenaoSe => "SENAOSE",
            Senao => "SENAO",
            Para => "PARA",
            Enquanto => "ENQUANTO",
            Faca => "FACA",
            Retorna => "RETORNA",
            Rotina => "ROTINA",
            Etapa => "ETAPA",
            Funcao => "FUNCAO",
            Logico => "LOGICO",
            Nulo => "NULO",
            Tipo => "TIPO",
            OpSoma => "OPSOMA",
            OpMult => "OPMULT",
            OpRel => "OPREL",
            OpRelIgual => "OPRELIGUAL",
            Igual => "IGUAL",
            Diferente => "DIFERENTE",
            E => "E",
            Ou => "OU",
            Nao => "NAO",
            Atribuicao => "ATRIBUICAO",
            AbrePar => "ABREPAR",
            FechaPar => "FECHAPAR",
            AbreChave => "ABRECHAVE",
            FechaChave => "FECHACHAVE",
            Sep => "SEP",
            PontoVirgula => "PONTOVIRGULA",
            Id => "ID",
            Inteiro => "INTEIRO",
            Texto => "TEXTO",
            Duracao => "DURACAO",
            Data => "DATA",
            Coment => "COMENT",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub tipo: Tok,
    /// Trecho em bytes no fonte.
    pub ini: u32,
    pub fim: u32,
    pub linha: u32,
    pub coluna: u32,
}

impl Token {
    pub fn lexema<'a>(&self, src: &'a str) -> &'a str {
        &src[self.ini as usize..self.fim as usize]
    }
}

pub struct Lexado {
    pub tokens: Vec<Token>,
    pub diagnosticos: Vec<Diagnostico>,
}

pub const TIPOS: [&str; 11] = [
    "num",
    "texto",
    "logico",
    "enum",
    "data",
    "duracao",
    "objeto",
    "mensagem",
    "resultado",
    "evento",
    "intensidade",
];

fn palavra(w: &str) -> Option<Tok> {
    Some(match w {
        "para" => Tok::Para,
        "enquanto" => Tok::Enquanto,
        "faca" => Tok::Faca,
        "retorna" => Tok::Retorna,
        "rotina" => Tok::Rotina,
        "etapa" => Tok::Etapa,
        "funcao" => Tok::Funcao,
        "verdadeiro" | "falso" => Tok::Logico,
        "nulo" => Tok::Nulo,
        _ if TIPOS.contains(&w) => Tok::Tipo,
        _ => return None,
    })
}

struct Lexer<'a> {
    src: &'a str,
    b: &'a [u8],
    i: usize,
    linha: u32,
    col: u32,
    tokens: Vec<Token>,
    diags: Vec<Diagnostico>,
}

fn eh_palavra(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

impl<'a> Lexer<'a> {
    fn atual(&self) -> Option<char> {
        self.src[self.i..].chars().next()
    }

    fn byte(&self, k: usize) -> Option<u8> {
        self.b.get(self.i + k).copied()
    }

    /// Avança um ponto de código, atualizando linha e coluna.
    #[inline]
    fn avanca(&mut self) {
        // Caminho rápido: quase todo fonte é ASCII, e aí um byte é um ponto de código.
        if let Some(&b) = self.b.get(self.i)
            && b < 0x80
        {
            self.i += 1;
            if b == b'\n' {
                self.linha += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            return;
        }
        let Some(c) = self.atual() else { return };
        self.i += c.len_utf8();
        if c == '\n' {
            self.linha += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
    }

    fn avanca_n(&mut self, n: usize) {
        for _ in 0..n {
            self.avanca();
        }
    }

    fn token(&mut self, tipo: Tok, ini: usize, linha: u32, coluna: u32) {
        self.tokens.push(Token {
            tipo,
            ini: ini as u32,
            fim: self.i as u32,
            linha,
            coluna,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn erro(
        &mut self,
        codigo: &'static str,
        ini: usize,
        fim: usize,
        linha: u32,
        coluna: u32,
        mensagem: String,
        sugestao: Option<Sugestao>,
    ) {
        self.diags.push(Diagnostico {
            codigo,
            mensagem,
            ini,
            fim,
            linha,
            coluna,
            fase: Fase::Lexico,
            sugestao,
        });
    }

    fn fim_de_linha(&self, mut k: usize) -> bool {
        k += self.i;
        match self.b.get(k) {
            None | Some(b'\n') => true,
            Some(b'\r') => self.b.get(k + 1) == Some(&b'\n'),
            _ => false,
        }
    }

    fn rodar(&mut self) {
        while self.i < self.b.len() {
            let (ini, linha, coluna) = (self.i, self.linha, self.col);
            let c = self.b[self.i];
            match c {
                b' ' | b'\t' | b'\n' => self.avanca(),
                b'\r' if self.byte(1) == Some(b'\n') => self.avanca_n(2),
                b'"' => self.texto(),
                b'#' => {
                    while !self.fim_de_linha(0) {
                        self.avanca();
                    }
                    self.token(Tok::Coment, ini, linha, coluna);
                }
                b'0'..=b'9' => self.numero(),
                b'a'..=b'z' | b'A'..=b'Z' => self.identificador(),
                _ => self.operador(),
            }
        }
    }

    fn texto(&mut self) {
        let (ini, linha, coluna) = (self.i, self.linha, self.col);
        self.avanca();
        let mut escape_ruim = false;
        loop {
            if self.fim_de_linha(0) {
                // Sem aspa final: sugere fechar no fim da linha (antes de um ";" final).
                let fim = self.i;
                let corpo = &self.src[ini..fim];
                let sem_espaco = corpo.trim_end();
                let pos = if sem_espaco.ends_with(';') && sem_espaco.len() > 1 {
                    ini + sem_espaco.len() - 1
                } else {
                    ini + sem_espaco.len()
                };
                let titulo = if pos < ini + sem_espaco.len() {
                    "Fechar a aspa antes do ;"
                } else {
                    "Fechar a aspa no fim da linha"
                };
                self.erro(
                    "E_LEX_TEXTO",
                    ini,
                    ini + 1,
                    linha,
                    coluna,
                    "texto sem aspa final nesta linha".into(),
                    Some(Sugestao::troca(titulo, pos, pos, "\"")),
                );
                return;
            }
            match self.b[self.i] {
                b'"' => {
                    self.avanca();
                    break;
                }
                b'\\' => {
                    let (bi, bl, bc) = (self.i, self.linha, self.col);
                    self.avanca();
                    match self.atual() {
                        Some('"' | '\\' | 'n') => self.avanca(),
                        _ if self.fim_de_linha(0) => {}
                        Some(e) => {
                            escape_ruim = true;
                            let sug = if e == 't' {
                                Sugestao::troca("Trocar \\t por um espaço", bi, bi + 2, " ")
                            } else {
                                Sugestao::troca("Remover a barra", bi, bi + 1, "")
                            };
                            self.erro(
                                "E_LEX_ESCAPE",
                                bi,
                                bi + 1 + e.len_utf8(),
                                bl,
                                bc,
                                format!("escape \\{e} não existe; use \\\", \\\\ ou \\n"),
                                Some(sug),
                            );
                            self.avanca();
                        }
                        None => {}
                    }
                }
                _ => self.avanca(),
            }
        }
        if !escape_ruim {
            self.token(Tok::Texto, ini, linha, coluna);
        }
    }

    fn numero(&mut self) {
        let (ini, linha, coluna) = (self.i, self.linha, self.col);
        while self.i < self.b.len() && eh_palavra(self.b[self.i]) {
            self.avanca();
        }
        let cand = &self.src[ini..self.i];
        if cand.bytes().all(|b| b.is_ascii_digit()) {
            let sig = cand.trim_start_matches('0');
            if sig.len() > 10 || (sig.len() == 10 && sig > "2147483647") {
                self.erro(
                    "E_LEX_LITERAL",
                    ini,
                    self.i,
                    linha,
                    coluna,
                    format!("o inteiro {cand} passa do máximo 2147483647"),
                    None,
                );
            } else {
                self.token(Tok::Inteiro, ini, linha, coluna);
            }
        } else if duracao_valida(cand) {
            self.token(Tok::Duracao, ini, linha, coluna);
        } else {
            let sug = sugerir_duracao(cand)
                .map(|s| Sugestao::troca(format!("Escrever {s}"), ini, self.i, s));
            self.erro(
                "E_LEX_LITERAL",
                ini,
                self.i,
                linha,
                coluna,
                format!("literal malformado: {cand}; uma duração usa h, min e s, nessa ordem e sem repetir"),
                sug,
            );
        }
    }

    fn identificador(&mut self) {
        let (ini, linha, coluna) = (self.i, self.linha, self.col);
        while self.i < self.b.len() && eh_palavra(self.b[self.i]) {
            self.avanca();
        }
        let w = &self.src[ini..self.i];
        match (w, self.byte(0)) {
            ("qjo", Some(b'?')) => {
                self.avanca();
                self.token(Tok::SenaoSe, ini, linha, coluna)
            }
            ("q", Some(b'?')) => {
                self.avanca();
                self.token(Tok::Se, ini, linha, coluna)
            }
            ("jo", Some(b'!')) => {
                self.avanca();
                self.token(Tok::Senao, ini, linha, coluna)
            }
            ("q" | "qjo" | "jo", _) => {
                let certo = if w == "jo" {
                    "jo!"
                } else if w == "q" {
                    "q?"
                } else {
                    "qjo?"
                };
                let fim = self.i;
                self.erro(
                    "E_LEX_RESERVADA",
                    ini,
                    fim,
                    linha,
                    coluna,
                    format!("{w} é reservado e só existe como {certo}"),
                    Some(Sugestao::troca(
                        format!("Escrever {certo}"),
                        fim,
                        fim,
                        &certo[w.len()..],
                    )),
                );
            }
            ("data", Some(b'"')) => self.data(ini, linha, coluna),
            _ => {
                let t = palavra(w).unwrap_or(Tok::Id);
                self.token(t, ini, linha, coluna);
            }
        }
    }

    fn data(&mut self, ini: usize, linha: u32, coluna: u32) {
        self.avanca(); // aspa
        let corpo_ini = self.i;
        let mut fechou = false;
        while !self.fim_de_linha(0) {
            if self.b[self.i] == b'"' {
                fechou = true;
                break;
            }
            self.avanca();
        }
        let corpo = &self.src[corpo_ini..self.i];
        if fechou {
            self.avanca();
        }
        if fechou && formato_data(corpo) {
            self.token(Tok::Data, ini, linha, coluna);
            return;
        }
        let sug = normalizar_data(corpo).map(|d| {
            let novo = format!("data\"{d}\"");
            Sugestao::troca(format!("Escrever {novo}"), ini, self.i, novo)
        });
        let msg = if fechou {
            format!(
                "data malformada: use data\"AAAA-MM-DD HH:MM:SS\" ou data\"HH:MM:SS\", veio data\"{corpo}\""
            )
        } else {
            "data sem aspa final nesta linha".to_string()
        };
        self.erro("E_LEX_DATA", ini, self.i, linha, coluna, msg, sug);
    }

    fn operador(&mut self) {
        let (ini, linha, coluna) = (self.i, self.linha, self.col);
        let dois = self.src.get(self.i..self.i + 2).unwrap_or("");
        let t2 = match dois {
            "<=" | ">=" => Some(Tok::OpRelIgual),
            "==" => Some(Tok::Igual),
            "!=" => Some(Tok::Diferente),
            "&&" => Some(Tok::E),
            "||" => Some(Tok::Ou),
            _ => None,
        };
        if let Some(t) = t2 {
            self.avanca_n(2);
            return self.token(t, ini, linha, coluna);
        }
        let t1 = match self.b[self.i] {
            b'+' | b'-' => Some(Tok::OpSoma),
            b'*' | b'/' | b'%' => Some(Tok::OpMult),
            b'<' | b'>' => Some(Tok::OpRel),
            b'!' => Some(Tok::Nao),
            b'=' => Some(Tok::Atribuicao),
            b'(' => Some(Tok::AbrePar),
            b')' => Some(Tok::FechaPar),
            b'{' => Some(Tok::AbreChave),
            b'}' => Some(Tok::FechaChave),
            b',' => Some(Tok::Sep),
            b';' => Some(Tok::PontoVirgula),
            _ => None,
        };
        if let Some(t) = t1 {
            self.avanca();
            return self.token(t, ini, linha, coluna);
        }
        let c = self.atual().unwrap_or('\0');
        self.avanca();
        let sug =
            sugerir_simbolo(c).map(|(titulo, troca)| Sugestao::troca(titulo, ini, self.i, troca));
        let desc = match c {
            '\r' => "CR sozinho não é quebra de linha (use LF ou CRLF)".to_string(),
            c if c.is_control() => format!(
                "caractere de controle U+{:04X} não pertence à linguagem",
                c as u32
            ),
            c => format!("o símbolo {c} não pertence à linguagem"),
        };
        self.erro("E_LEX_SIMBOLO", ini, self.i, linha, coluna, desc, sug);
    }
}

/// `(N h)? (N min)? (N s)?` com ao menos um grupo, sem nada depois.
pub fn duracao_valida(c: &str) -> bool {
    duracao_segundos(c).is_some()
}

/// Total em segundos (saturado em `u64::MAX`), se a grafia é válida.
pub fn duracao_segundos(c: &str) -> Option<u64> {
    let b = c.as_bytes();
    let (mut i, mut ultima, mut total, mut grupos) = (0usize, -1i32, 0u64, 0);
    while i < b.len() {
        let ini = i;
        let mut n: u64 = 0;
        while i < b.len() && b[i].is_ascii_digit() {
            n = n.saturating_mul(10).saturating_add((b[i] - b'0') as u64);
            i += 1;
        }
        if i == ini {
            return None;
        }
        let (unidade, fator, tam) = if c[i..].starts_with("min") {
            (1, 60, 3)
        } else if c[i..].starts_with('h') {
            (0, 3600, 1)
        } else if c[i..].starts_with('s') {
            (2, 1, 1)
        } else {
            return None;
        };
        if unidade <= ultima {
            return None;
        }
        ultima = unidade;
        i += tam;
        total = total.saturating_add(n.saturating_mul(fator));
        grupos += 1;
    }
    (grupos > 0).then_some(total)
}

fn sugerir_duracao(c: &str) -> Option<String> {
    // Troca unidades escritas por extenso: 30seg -> 30s, 2mins -> 2min, 1hora -> 1h.
    let por_extenso = || -> Option<String> {
        let mut out = String::new();
        let mut resto = c;
        while !resto.is_empty() {
            let d = resto.bytes().take_while(u8::is_ascii_digit).count();
            if d == 0 {
                return None;
            }
            let (num, r) = resto.split_at(d);
            let l = r.bytes().take_while(|b| b.is_ascii_alphabetic()).count();
            let (un, r2) = r.split_at(l);
            let un = match un.to_ascii_lowercase().as_str() {
                "h" | "hr" | "hrs" | "hora" | "horas" => "h",
                "m" | "mn" | "mi" | "min" | "mins" | "minuto" | "minutos" => "min",
                "s" | "seg" | "segs" | "sec" | "segundo" | "segundos" => "s",
                _ => return None,
            };
            out.push_str(num);
            out.push_str(un);
            resto = r2;
        }
        (out != c && duracao_valida(&out)).then_some(out)
    };
    // Senão, o maior prefixo válido: 2minx -> 2min.
    por_extenso().or_else(|| {
        (2..c.len())
            .rev()
            .map(|k| &c[..k])
            .find(|p| duracao_valida(p))
            .map(str::to_string)
    })
}

fn formato_data(c: &str) -> bool {
    let b = c.as_bytes();
    let dig = |r: std::ops::Range<usize>| r.into_iter().all(|k| b[k].is_ascii_digit());
    match b.len() {
        8 => dig(0..2) && b[2] == b':' && dig(3..5) && b[5] == b':' && dig(6..8),
        19 => {
            dig(0..4)
                && b[4] == b'-'
                && dig(5..7)
                && b[7] == b'-'
                && dig(8..10)
                && b[10] == b' '
                && dig(11..13)
                && b[13] == b':'
                && dig(14..16)
                && b[16] == b':'
                && dig(17..19)
        }
        _ => false,
    }
}

fn normalizar_data(c: &str) -> Option<String> {
    let c = c.trim();
    let hora = |h: &str| -> Option<String> {
        let p: Vec<&str> = h.split(':').collect();
        if !(2..=3).contains(&p.len())
            || p.iter()
                .any(|x| x.is_empty() || x.len() > 2 || !x.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        let s = if p.len() == 3 { p[2] } else { "0" };
        Some(format!("{:0>2}:{:0>2}:{:0>2}", p[0], p[1], s))
    };
    let r = match c.split_once(' ') {
        Some((d, h)) => {
            let p: Vec<&str> = d.split('-').collect();
            if p.len() != 3
                || p[0].len() != 4
                || p.iter()
                    .any(|x| x.is_empty() || !x.bytes().all(|b| b.is_ascii_digit()))
            {
                return None;
            }
            format!("{}-{:0>2}-{:0>2} {}", p[0], p[1], p[2], hora(h.trim())?)
        }
        None => hora(c)?,
    };
    (formato_data(&r)).then_some(r)
}

fn sugerir_simbolo(c: char) -> Option<(String, String)> {
    let troca = match c {
        '≠' => "!=",
        '≤' => "<=",
        '≥' => ">=",
        '&' => "&&",
        '|' => "||",
        '“' | '”' | '„' | '\'' | '‘' | '’' => "\"",
        '×' => "*",
        '÷' => "/",
        '−' | '–' | '—' => "-",
        _ => {
            let s = sem_acento(c)?;
            return Some((format!("Escrever sem acento: {s}"), s.to_string()));
        }
    };
    Some((format!("Trocar por {troca}"), troca.to_string()))
}

fn sem_acento(c: char) -> Option<char> {
    Some(match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        _ => return None,
    })
}

/// Analisa o fonte inteiro. Comentários aparecem como `Tok::Coment`.
pub fn lex(src: &str) -> Lexado {
    let mut l = Lexer {
        src,
        b: src.as_bytes(),
        i: 0,
        linha: 1,
        col: 1,
        tokens: Vec::with_capacity(src.len() / 4),
        diags: vec![],
    };
    l.rodar();
    Lexado {
        tokens: l.tokens,
        diagnosticos: l.diags,
    }
}

/// Decodifica um lexema `TEXTO` (com aspas) para o valor.
pub fn decodificar_texto(lexema: &str) -> String {
    let corpo = &lexema[1..lexema.len() - 1];
    let mut out = String::with_capacity(corpo.len());
    let mut it = corpo.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn tipos(src: &str) -> Vec<(Tok, &str)> {
        lex(src)
            .tokens
            .iter()
            .map(|t| (t.tipo, t.lexema(src)))
            .collect()
    }

    #[test]
    fn duracoes() {
        for ok in [
            "10s",
            "2min",
            "1h30min20s",
            "1h20s",
            "0s",
            "90s",
            "2147483648s",
        ] {
            assert!(duracao_valida(ok), "{ok}");
        }
        for ruim in ["30seg", "1h2h", "30s1min", "2minx", "2min_", "h", "1"] {
            assert!(!duracao_valida(ruim), "{ruim}");
        }
        assert_eq!(sugerir_duracao("30seg").as_deref(), Some("30s"));
        assert_eq!(sugerir_duracao("2minx").as_deref(), Some("2min"));
        assert_eq!(sugerir_duracao("1mn").as_deref(), Some("1min"));
    }

    #[test]
    fn pontuadas_e_reservadas() {
        assert_eq!(
            tipos("q? qjo? jo!"),
            vec![(Tok::Se, "q?"), (Tok::SenaoSe, "qjo?"), (Tok::Senao, "jo!")]
        );
        let r = lex("meuqjo? q");
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(
            r.diagnosticos.iter().map(|d| d.codigo).collect::<Vec<_>>(),
            ["E_LEX_SIMBOLO", "E_LEX_RESERVADA"]
        );
    }

    #[test]
    fn colunas_em_pontos_de_codigo() {
        let r = lex("texto a = \"ação\"; @");
        let d = &r.diagnosticos[0];
        assert_eq!((d.linha, d.coluna), (1, 19));
    }

    #[test]
    fn data() {
        assert_eq!(
            tipos("data\"07:30:00\" data x"),
            vec![
                (Tok::Data, "data\"07:30:00\""),
                (Tok::Tipo, "data"),
                (Tok::Id, "x")
            ]
        );
        let r = lex("data\"7:30\";");
        assert_eq!(r.diagnosticos[0].codigo, "E_LEX_DATA");
        assert_eq!(
            r.diagnosticos[0].sugestao.as_ref().unwrap().titulo,
            "Escrever data\"07:30:00\""
        );
        assert_eq!(r.tokens[0].tipo, Tok::PontoVirgula);
    }
}
