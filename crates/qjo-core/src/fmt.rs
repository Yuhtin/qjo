//! Formatador: indentação de 4 espaços, um comando por linha, espaços em volta dos operadores.
//! Trabalha sobre os tokens, então preserva comentários e funciona mesmo com erros de sintaxe.

use crate::lexer::{Tok, Token, lex};

struct Fmt {
    out: String,
    linha: String,
    nivel: usize,
    parens: usize,
    /// Pilha de blocos abertos: é bloco de `faca`? é enum (em linha)?
    blocos: Vec<(bool, bool)>,
}

impl Fmt {
    fn quebra(&mut self) {
        let l = self.linha.trim_end();
        if !l.is_empty() {
            for _ in 0..self.nivel {
                self.out.push_str("    ");
            }
            self.out.push_str(l);
        }
        self.out.push('\n');
        self.linha.clear();
    }

    fn em_branco(&mut self) {
        if !self.linha.trim().is_empty() {
            self.quebra();
        }
        if !self.out.ends_with("\n\n") && !self.out.ends_with("{\n") && !self.out.is_empty() {
            self.out.push('\n');
        }
    }
}

fn termina_valor(t: Tok) -> bool {
    matches!(
        t,
        Tok::Id
            | Tok::Inteiro
            | Tok::Duracao
            | Tok::Data
            | Tok::Texto
            | Tok::Logico
            | Tok::Nulo
            | Tok::FechaPar
    )
}

fn espaco_entre(p: Token, t: Token, src: &str, unario_antes: bool) -> bool {
    use Tok::*;
    let (pt, tt) = (p.tipo, t.tipo);
    if matches!(pt, AbrePar) || matches!(tt, FechaPar | Sep | PontoVirgula) {
        return false;
    }
    if unario_antes {
        return false;
    }
    if tt == AbrePar {
        return matches!(
            pt,
            Se | SenaoSe
                | Enquanto
                | Para
                | OpSoma
                | OpMult
                | OpRel
                | OpRelIgual
                | Igual
                | Diferente
                | E
                | Ou
                | Atribuicao
                | Sep
                | Retorna
        );
    }
    let _ = src;
    true
}

/// Formata o fonte. `None` se há erro léxico (sem tokens confiáveis, melhor não mexer).
pub fn formatar(src: &str) -> Option<String> {
    let lx = lex(src);
    if !lx.diagnosticos.is_empty() {
        return None;
    }
    let t = &lx.tokens;
    let mut f = Fmt {
        out: String::new(),
        linha: String::new(),
        nivel: 0,
        parens: 0,
        blocos: vec![],
    };
    let mut unario = false;
    let mut ultimo: Option<Token> = None; // último token que não é comentário
    let mut pular = usize::MAX;
    // Comentário logo depois de t[i], na mesma linha: vai junto antes da quebra.
    let junto = |i: usize| -> Option<usize> {
        let x = t.get(i + 1)?;
        (x.tipo == Tok::Coment && !src[t[i].fim as usize..x.ini as usize].contains('\n'))
            .then_some(i + 1)
    };
    for (i, &tok) in t.iter().enumerate() {
        if i == pular {
            continue;
        }
        let lexema = tok.lexema(src);
        let ant = if i > 0 { Some(t[i - 1]) } else { None };
        let quebras = ant
            .map(|a| src[a.fim as usize..tok.ini as usize].matches('\n').count())
            .unwrap_or(0);

        if tok.tipo == Tok::Coment {
            if quebras == 0 && !f.linha.trim().is_empty() {
                f.linha.push(' ');
                f.linha.push_str(lexema.trim_end());
                f.quebra();
            } else {
                if quebras >= 2 {
                    f.em_branco();
                } else if !f.linha.trim().is_empty() {
                    f.quebra();
                }
                f.linha.push_str(lexema.trim_end());
                f.quebra();
            }
            continue;
        }

        let em_enum = f.blocos.last().map(|b| b.1).unwrap_or(false);
        if tok.tipo == Tok::FechaChave && !em_enum {
            if !f.linha.trim().is_empty() {
                f.quebra();
            }
            f.nivel = f.nivel.saturating_sub(1);
        } else if f.linha.trim().is_empty() {
            if quebras >= 2 && !f.out.ends_with("{\n") && !f.out.is_empty() {
                f.em_branco();
            }
        } else if let Some(p) = ultimo
            && espaco_entre(p, tok, src, unario)
        {
            f.linha.push(' ');
        }

        f.linha.push_str(lexema);

        // `-` é unário quando não vem depois de um valor.
        unario = match tok.tipo {
            Tok::Nao => true,
            Tok::OpSoma if lexema == "-" => !ultimo.map(|p| termina_valor(p.tipo)).unwrap_or(false),
            _ => false,
        };

        match tok.tipo {
            Tok::AbrePar => f.parens += 1,
            Tok::FechaPar => f.parens = f.parens.saturating_sub(1),
            Tok::PontoVirgula if f.parens == 0 => {
                if let Some(c) = junto(i) {
                    f.linha.push(' ');
                    f.linha.push_str(t[c].lexema(src).trim_end());
                    pular = c;
                }
                f.quebra()
            }
            Tok::AbreChave => {
                let eh_faca = ultimo.map(|p| p.tipo == Tok::Faca).unwrap_or(false);
                let eh_enum =
                    i >= 2 && t[i - 2].tipo == Tok::Tipo && t[i - 2].lexema(src) == "enum";
                f.blocos.push((eh_faca, eh_enum));
                if !eh_enum {
                    if let Some(c) = junto(i) {
                        f.linha.push(' ');
                        f.linha.push_str(t[c].lexema(src).trim_end());
                        pular = c;
                    }
                    f.quebra();
                    f.nivel += 1;
                }
            }
            Tok::FechaChave => {
                let (eh_faca, eh_enum) = f.blocos.pop().unwrap_or((false, false));
                if eh_enum {
                    // "{ A, B }": o espaço antes do "}" vem do espaco_entre.
                }
                let prox = t[i + 1..]
                    .iter()
                    .find(|x| x.tipo != Tok::Coment)
                    .map(|x| x.tipo);
                let continua = match prox {
                    Some(Tok::Senao | Tok::SenaoSe) => true,
                    Some(Tok::Enquanto) => eh_faca,
                    Some(Tok::PontoVirgula) => eh_enum,
                    _ => false,
                };
                if !continua {
                    if let Some(c) = junto(i) {
                        f.linha.push(' ');
                        f.linha.push_str(t[c].lexema(src).trim_end());
                        pular = c;
                    }
                    f.quebra();
                }
            }
            _ => {}
        }
        ultimo = Some(tok);
    }
    if !f.linha.trim().is_empty() {
        f.quebra();
    }
    let mut out = f.out.trim_end().to_string();
    out.push('\n');
    if out.trim().is_empty() {
        return Some(String::new());
    }
    Some(out)
}

#[cfg(test)]
mod testes {
    use super::formatar;

    #[test]
    fn basico() {
        let src = "data h=data\"08:00:00\";mensagem m=\"Oi\";\nrotina(\"R\",h){etapa(\"E\"){INSTRUIR(m);num x=-1+2*3; q?(x!=5&&!falso){ESPERAR(1s);}jo!{retorna;}}}";
        let esperado = "data h = data\"08:00:00\";\nmensagem m = \"Oi\";\nrotina(\"R\", h) {\n    etapa(\"E\") {\n        INSTRUIR(m);\n        num x = -1 + 2 * 3;\n        q? (x != 5 && !falso) {\n            ESPERAR(1s);\n        } jo! {\n            retorna;\n        }\n    }\n}\n";
        let r = formatar(src).unwrap();
        assert_eq!(r, esperado);
        assert_eq!(formatar(&r).unwrap(), r, "idempotente");
    }

    #[test]
    fn faca_enum_comentarios_e_linhas_em_branco() {
        let src = "enum Turno{MANHA,NOITE}\n\n\n# rotina\nrotina(\"R\",data\"08:00:00\"){ # topo\nfaca{ESPERAR(1s);}enquanto(falso);\n\npara(num i=0;i<3;i=i+1){ESPERAR(1s);}\n}\n";
        let esperado = "enum Turno { MANHA, NOITE }\n\n# rotina\nrotina(\"R\", data\"08:00:00\") { # topo\n    faca {\n        ESPERAR(1s);\n    } enquanto (falso);\n\n    para (num i = 0; i < 3; i = i + 1) {\n        ESPERAR(1s);\n    }\n}\n";
        let r = formatar(src).unwrap();
        assert_eq!(r, esperado);
        assert_eq!(formatar(&r).unwrap(), r, "idempotente");
    }
}
