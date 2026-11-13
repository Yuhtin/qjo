//! Roda tests/conformance: cada `.qjo` tem um `.esperado` ao lado.
//! `QJO_ATUALIZAR=1 cargo test` reescreve os esperados (revise o diff antes de commitar).

use std::path::{Path, PathBuf};

use qjo_core::{Tok, analisar, interp::ler_respostas, lex, rodar};

pub fn saida(src: &str) -> String {
    let mut out = String::new();
    if src.lines().any(|l| l.trim() == "#! modo: tokens") {
        let lx = lex(src);
        for t in lx.tokens.iter().filter(|t| t.tipo != Tok::Coment) {
            out += &format!(
                "{}:{} {} {}\n",
                t.linha,
                t.coluna,
                t.tipo.nome(),
                t.lexema(src)
            );
        }
        for d in &lx.diagnosticos {
            out += &format!("erro {}:{} {}\n", d.linha, d.coluna, d.codigo);
        }
        return out;
    }
    let a = analisar(src);
    if !a.ok() {
        for d in &a.diagnosticos {
            out += &format!("erro {}:{} {}\n", d.linha, d.coluna, d.codigo);
        }
        return out;
    }
    let respostas = src
        .lines()
        .find_map(|l| l.strip_prefix("#! respostas:"))
        .unwrap_or("");
    let respostas = ler_respostas(respostas).expect("respostas inválidas no teste");
    let (linhas, _) = rodar(src, &respostas).unwrap();
    for l in linhas {
        out += &l;
        out.push('\n');
    }
    out
}

fn casos(dir: &Path, v: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            casos(&p, v);
        } else if p.extension().is_some_and(|x| x == "qjo") {
            v.push(p);
        }
    }
}

#[test]
fn conformidade() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance");
    let mut v = vec![];
    casos(&raiz, &mut v);
    v.sort();
    assert!(!v.is_empty(), "nenhum caso em {}", raiz.display());
    let atualizar = std::env::var_os("QJO_ATUALIZAR").is_some();
    let mut falhas = vec![];
    for p in &v {
        let src = std::fs::read_to_string(p).unwrap();
        let obtido = saida(&src);
        let esp = p.with_extension("esperado");
        if atualizar {
            std::fs::write(&esp, &obtido).unwrap();
            continue;
        }
        let esperado = std::fs::read_to_string(&esp).unwrap_or_default();
        if obtido != esperado {
            falhas.push(format!(
                "{}\n--- esperado\n{esperado}--- obtido\n{obtido}",
                p.strip_prefix(&raiz).unwrap().display()
            ));
        }
    }
    assert!(
        falhas.is_empty(),
        "{} de {} casos falharam:\n\n{}",
        falhas.len(),
        v.len(),
        falhas.join("\n")
    );
    println!("{} casos de conformidade ok", v.len());
}

#[test]
fn exemplos_sem_erros() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for e in std::fs::read_dir(&raiz).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "qjo") {
            let src = std::fs::read_to_string(&p).unwrap();
            let a = analisar(&src);
            assert!(a.ok(), "{}: {:?}", p.display(), a.diagnosticos);
            let f = qjo_core::fmt::formatar(&src).unwrap();
            assert_eq!(f, src, "{} não está formatado", p.display());
        }
    }
}
