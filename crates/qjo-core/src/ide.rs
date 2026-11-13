//! Consultas para editores: símbolo sob o cursor, definição, referências, renomear,
//! autocompletar, dica e esboço do arquivo. Posições em bytes.

use crate::Analise;
use crate::ast::*;
use crate::diag::Edicao;
use crate::lexer::{TIPOS, Tok};
use crate::sema::Especie;

/// Símbolo sob o deslocamento: um uso ou a própria declaração.
pub fn simbolo_em(a: &Analise, off: u32) -> Option<usize> {
    a.info
        .usos
        .iter()
        .find(|(sp, _)| sp.contem(off))
        .map(|(_, s)| *s)
        .or_else(|| {
            a.info
                .simbolos
                .iter()
                .position(|s| s.def.is_some_and(|d| d.contem(off)))
        })
}

pub fn definicao(a: &Analise, off: u32) -> Option<Span> {
    a.info.simbolos[simbolo_em(a, off)?].def
}

pub fn referencias(a: &Analise, off: u32, com_declaracao: bool) -> Vec<Span> {
    let Some(s) = simbolo_em(a, off) else {
        return vec![];
    };
    let mut v: Vec<Span> = a
        .info
        .usos
        .iter()
        .filter(|(_, x)| *x == s)
        .map(|(sp, _)| *sp)
        .collect();
    if com_declaracao && let Some(d) = a.info.simbolos[s].def {
        v.push(d);
    }
    v.sort_by_key(|sp| sp.ini);
    v.dedup();
    v
}

fn nome_valido(n: &str) -> bool {
    let mut c = n.chars();
    c.next().is_some_and(|x| x.is_ascii_alphabetic())
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_')
        && !TIPOS.contains(&n)
        && !matches!(
            n,
            "q" | "qjo"
                | "jo"
                | "para"
                | "enquanto"
                | "faca"
                | "retorna"
                | "rotina"
                | "etapa"
                | "funcao"
                | "verdadeiro"
                | "falso"
                | "nulo"
        )
}

/// O trecho que pode ser renomeado sob o cursor, e o nome atual.
pub fn preparar_renomear(a: &Analise, off: u32) -> Option<(Span, String)> {
    let s = simbolo_em(a, off)?;
    let sim = &a.info.simbolos[s];
    sim.def?;
    let sp = a
        .info
        .usos
        .iter()
        .find(|(sp, x)| *x == s && sp.contem(off))
        .map(|(sp, _)| *sp)
        .or(sim.def.filter(|d| d.contem(off)))?;
    Some((sp, sim.nome.clone()))
}

pub fn renomear(a: &Analise, off: u32, novo: &str) -> Result<Vec<Edicao>, String> {
    let Some(s) = simbolo_em(a, off) else {
        return Err("não há nome aqui".into());
    };
    let sim = &a.info.simbolos[s];
    if sim.def.is_none() {
        return Err(format!(
            "{} faz parte da linguagem e não pode ser renomeado",
            sim.nome
        ));
    }
    if !nome_valido(novo) {
        return Err(format!(
            "\"{novo}\" não é um nome válido (letra, depois letras, dígitos ou _)"
        ));
    }
    let conflito = a.info.simbolos.iter().any(|o| {
        o.nome == novo && (o.visivel.ini <= sim.visivel.fim && sim.visivel.ini <= o.visivel.fim)
    });
    if conflito {
        return Err(format!("já existe \"{novo}\" visível aqui"));
    }
    Ok(referencias(a, off, true)
        .into_iter()
        .map(|sp| Edicao {
            ini: sp.ini as usize,
            fim: sp.fim as usize,
            texto: novo.to_string(),
        })
        .collect())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EspecieItem {
    Palavra,
    Tipo,
    Primitiva,
    Funcao,
    Variavel,
    Constante,
    ValorEnum,
    Enum,
    Trecho,
}

#[derive(Clone, Debug)]
pub struct ItemCompletar {
    pub rotulo: String,
    pub especie: EspecieItem,
    pub detalhe: String,
    /// Texto a inserir no formato de snippet LSP (`$1`, `$0`), se diferente do rótulo.
    pub snippet: Option<String>,
}

const TRECHOS: [(&str, &str, &str); 8] = [
    (
        "rotina",
        "rotina(\"${1:Nome}\", ${2:horario}) {\n\t$0\n}",
        "rotina com horário",
    ),
    (
        "etapa",
        "etapa(\"${1:Nome}\") {\n\t$0\n}",
        "etapa da rotina",
    ),
    ("q?", "q? (${1:condicao}) {\n\t$0\n}", "se"),
    (
        "q? jo!",
        "q? (${1:condicao}) {\n\t$2\n} jo! {\n\t$0\n}",
        "se … senão",
    ),
    (
        "enquanto",
        "enquanto (${1:condicao}) {\n\t$0\n}",
        "laço enquanto",
    ),
    (
        "faca",
        "faca {\n\t$0\n} enquanto (${1:condicao});",
        "laço faca … enquanto",
    ),
    (
        "para",
        "para (num ${1:i} = 0; ${1:i} < ${2:3}; ${1:i} = ${1:i} + 1) {\n\t$0\n}",
        "laço contado",
    ),
    (
        "confirmar ou cancelar",
        "INDICAR(${1:objeto});\nINSTRUIR(${2:mensagem});\nresultado ${3:r} = AGUARDAR_CONFIRMACAO(${4:1min});\nPARAR_INDICACAO(${1:objeto});\nq? (${3:r} != CONFIRMADO) {\n\tREGISTRAR(ROTINA_INCOMPLETA);\n\tretorna;\n}",
        "indica, instrui, espera e cancela a rotina se não confirmar",
    ),
];

/// Itens para autocompletar no deslocamento: o que está visível ali, palavras e trechos.
pub fn completar(a: &Analise, off: u32) -> Vec<ItemCompletar> {
    // Depois de um tipo vem um nome novo: nada a sugerir.
    if let Some(ant) = a
        .tokens
        .iter()
        .rev()
        .find(|t| t.fim < off && t.tipo != Tok::Coment)
        && ant.tipo == Tok::Tipo
    {
        return vec![];
    }
    let mut v = vec![];
    for s in &a.info.simbolos {
        if !s.visivel.contem(off) || s.def.is_some_and(|d| d.contem(off)) {
            continue;
        }
        let especie = match s.especie {
            Especie::Primitiva => EspecieItem::Primitiva,
            Especie::Funcao => EspecieItem::Funcao,
            Especie::Recurso | Especie::Horario => EspecieItem::Constante,
            Especie::ValorEnum => EspecieItem::ValorEnum,
            Especie::Enum => EspecieItem::Enum,
            _ => EspecieItem::Variavel,
        };
        let snippet = match s.especie {
            Especie::Primitiva | Especie::Funcao => {
                let sem_args = s.detalhe.contains("()");
                Some(if sem_args {
                    format!("{}()", s.nome)
                } else {
                    format!("{}($1)", s.nome)
                })
            }
            _ => None,
        };
        v.push(ItemCompletar {
            rotulo: s.nome.clone(),
            especie,
            detalhe: s.detalhe.clone(),
            snippet,
        });
    }
    for t in TIPOS {
        v.push(ItemCompletar {
            rotulo: t.into(),
            especie: EspecieItem::Tipo,
            detalhe: "tipo".into(),
            snippet: None,
        });
    }
    for p in [
        "retorna",
        "verdadeiro",
        "falso",
        "nulo",
        "qjo?",
        "jo!",
        "funcao",
        "enum",
    ] {
        v.push(ItemCompletar {
            rotulo: p.into(),
            especie: EspecieItem::Palavra,
            detalhe: "palavra reservada".into(),
            snippet: None,
        });
    }
    for (r, s, d) in TRECHOS {
        v.push(ItemCompletar {
            rotulo: r.into(),
            especie: EspecieItem::Trecho,
            detalhe: d.into(),
            snippet: Some(s.into()),
        });
    }
    v
}

fn doc_palavra(l: &str) -> Option<&'static str> {
    Some(match l {
        "q?" => "**q?** — *se*: executa o bloco quando a condição é verdadeira.",
        "qjo?" => "**qjo?** — *senão se*: testa outra condição quando as anteriores falharam.",
        "jo!" => "**jo!** — *senão*: executa quando nenhuma condição anterior foi verdadeira.",
        "rotina" => "**rotina**(\"Nome\", horário) { … } — o programa que o dispositivo executa.",
        "etapa" => {
            "**etapa**(\"Nome\") { … } — um passo da rotina. Ao terminar, apaga as indicações ligadas."
        }
        "retorna" => "**retorna;** — na rotina, cancela a rotina. Numa função, devolve o valor.",
        "para" => "**para** (num i = 0; i < n; i = i + 1) { … } — laço contado.",
        "enquanto" => "**enquanto** (condição) { … } — testa antes de cada volta.",
        "faca" => "**faca** { … } enquanto (condição); — executa ao menos uma vez.",
        "funcao" => "**funcao** [tipo] nome(tipo param, …) { … } — sem tipo, não devolve valor.",
        "nulo" => {
            "**nulo** — ausência de objeto, mensagem ou resultado. Primitivas que recebem nulo não fazem nada."
        }
        _ => return None,
    })
}

/// Dica para o deslocamento: trecho e texto em Markdown.
pub fn dica(a: &Analise, src: &str, off: u32) -> Option<(Span, String)> {
    if let Some(s) = simbolo_em(a, off) {
        let sim = &a.info.simbolos[s];
        let sp = a
            .info
            .usos
            .iter()
            .find(|(sp, x)| *x == s && sp.contem(off))
            .map(|(sp, _)| *sp)
            .or(sim.def)
            .unwrap_or_default();
        let mut md = format!(
            "```qjo\n{}\n```",
            if sim.detalhe.is_empty() {
                &sim.nome
            } else {
                &sim.detalhe
            }
        );
        if let Some((_, p)) = PRIMITIVAS.iter().find(|(n, _)| *n == sim.nome) {
            md += &format!("\n{}", p.descricao());
        }
        return Some((sp, md));
    }
    let t = a.tokens.iter().find(|t| t.ini <= off && off <= t.fim)?;
    let l = t.lexema(src);
    let md = match t.tipo {
        Tok::Duracao => {
            let s = crate::lexer::duracao_segundos(l)?;
            format!("duração de **{s} segundos**")
        }
        _ => doc_palavra(l)?.to_string(),
    };
    Some((Span::new(t.ini, t.fim), md))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EspecieEsboco {
    Rotina,
    Etapa,
    Funcao,
    Enum,
    Recurso,
    Horario,
}

#[derive(Clone, Debug)]
pub struct NoEsboco {
    pub nome: String,
    pub especie: EspecieEsboco,
    pub span: Span,
    pub selecao: Span,
    pub filhos: Vec<NoEsboco>,
}

fn etapas(b: &Bloco, v: &mut Vec<NoEsboco>) {
    for s in &b.cmds {
        match &s.k {
            StmtK::Etapa {
                nome, nome_span, ..
            } => v.push(NoEsboco {
                nome: nome.clone(),
                especie: EspecieEsboco::Etapa,
                span: s.span,
                selecao: *nome_span,
                filhos: vec![],
            }),
            StmtK::Se { ramos, senao } => {
                for (_, rb) in ramos {
                    etapas(rb, v);
                }
                if let Some(sb) = senao {
                    etapas(sb, v);
                }
            }
            StmtK::Enquanto { corpo, .. }
            | StmtK::Faca { corpo, .. }
            | StmtK::Para { corpo, .. } => etapas(corpo, v),
            _ => {}
        }
    }
}

/// Rotinas (com suas etapas), funções, enums, recursos e horários.
pub fn esboco(p: &Programa) -> Vec<NoEsboco> {
    p.itens
        .iter()
        .map(|i| match i {
            Item::Rotina(r) => {
                let mut filhos = vec![];
                etapas(&r.corpo, &mut filhos);
                NoEsboco {
                    nome: r.nome.clone(),
                    especie: EspecieEsboco::Rotina,
                    span: r.span,
                    selecao: r.nome_span,
                    filhos,
                }
            }
            Item::Funcao(f) => NoEsboco {
                nome: f.nome.clone(),
                especie: EspecieEsboco::Funcao,
                span: f.span,
                selecao: f.nome_span,
                filhos: vec![],
            },
            Item::Enum {
                nome,
                nome_span,
                span,
                ..
            } => NoEsboco {
                nome: nome.clone(),
                especie: EspecieEsboco::Enum,
                span: *span,
                selecao: *nome_span,
                filhos: vec![],
            },
            Item::Global {
                tipo,
                nome,
                nome_span,
                span,
                ..
            } => NoEsboco {
                nome: nome.clone(),
                especie: if tipo.nome == "data" {
                    EspecieEsboco::Horario
                } else {
                    EspecieEsboco::Recurso
                },
                span: *span,
                selecao: *nome_span,
                filhos: vec![],
            },
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::analisar;

    const SRC: &str = "data h = data\"08:00:00\";\nobjeto caixa = \"Caixa\";\nrotina(\"R\", h) {\n    etapa(\"E\") {\n        INDICAR(caixa);\n        num n = 1;\n        n = n + 1;\n    }\n}\n";

    #[test]
    fn definicao_referencias_renomear() {
        let a = analisar(SRC);
        let uso = SRC.find("INDICAR(caixa)").unwrap() as u32 + 9;
        let def = definicao(&a, uso).unwrap();
        assert_eq!(&SRC[def.ini as usize..def.fim as usize], "caixa");
        let n = SRC.rfind("n + 1").unwrap() as u32;
        assert_eq!(referencias(&a, n, true).len(), 3);
        let ed = renomear(&a, n, "contador").unwrap();
        assert_eq!(ed.len(), 3);
        assert!(renomear(&a, n, "caixa").is_err(), "conflito com recurso");
        assert!(renomear(&a, n, "num").is_err(), "palavra reservada");
        assert!(renomear(&a, uso - 3, "X").is_err(), "primitiva");
    }

    #[test]
    fn completar_respeita_escopo() {
        let a = analisar(SRC);
        let antes = SRC.find("INDICAR").unwrap() as u32;
        let rot: Vec<String> = completar(&a, antes).into_iter().map(|i| i.rotulo).collect();
        assert!(
            rot.contains(&"caixa".to_string()) && rot.contains(&"AGUARDAR_CONFIRMACAO".to_string())
        );
        assert!(!rot.contains(&"n".to_string()), "n ainda não foi declarado");
        let depois = SRC.rfind("n = n").unwrap() as u32;
        assert!(completar(&a, depois).iter().any(|i| i.rotulo == "n"));
    }

    #[test]
    fn esboco_e_dica() {
        let a = analisar(SRC);
        let e = esboco(&a.programa);
        assert_eq!(e.len(), 3);
        assert_eq!(e[2].filhos[0].nome, "E");
        let d = dica(&a, SRC, SRC.find("INDICAR").unwrap() as u32 + 2).unwrap();
        assert!(d.1.contains("INDICAR(objeto)"), "{}", d.1);
    }
}
