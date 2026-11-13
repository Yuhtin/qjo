//! Para cada caso de conformidade executável: gera C++, compila com o compilador do sistema
//! e confere que a saída do binário é igual ao `.esperado` (spec/05-cpp.md).
//! Sem compilador C++ disponível, o teste avisa e passa (a CI sempre tem g++).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use qjo_core::{analisar, cpp, interp::ler_respostas};

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
fn cpp_igual_ao_interpretador() {
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
    if Command::new(&cxx).arg("--version").output().is_err() {
        eprintln!("aviso: sem compilador C++ ({cxx}); teste do C++ pulado");
        return;
    }
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance");
    let alvo = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cpp");
    std::fs::create_dir_all(&alvo).unwrap();
    let mut v = vec![];
    casos(&raiz, &mut v);
    v.sort();
    let executaveis: Vec<(PathBuf, String)> = v
        .into_iter()
        .filter_map(|p| {
            let src = std::fs::read_to_string(&p).unwrap();
            let a = analisar(&src);
            (a.ok() && !src.contains("#! modo: tokens")).then(|| {
                let nome = p
                    .strip_prefix(&raiz)
                    .unwrap()
                    .to_string_lossy()
                    .replace(['/', '\\'], "_")
                    .replace(".qjo", "");
                std::fs::write(
                    alvo.join(format!("{nome}.cpp")),
                    cpp::gerar(&a.programa, &a.info, "teste.qjo"),
                )
                .unwrap();
                (p, nome)
            })
        })
        .collect();
    assert!(!executaveis.is_empty());

    let falhas: Vec<String> = std::thread::scope(|s| {
        let tarefas: Vec<_> = executaveis
            .iter()
            .map(|(p, nome)| {
                let (cxx, alvo) = (&cxx, &alvo);
                s.spawn(move || -> Option<String> {
                    let cpp = alvo.join(format!("{nome}.cpp"));
                    let bin = alvo.join(nome);
                    let c = Command::new(cxx)
                        .args(["-std=c++17", "-O1", "-Wall", "-Wextra", "-Werror", "-o"])
                        .arg(&bin)
                        .arg(&cpp)
                        .output()
                        .unwrap();
                    if !c.status.success() {
                        return Some(format!(
                            "{nome}: não compilou\n{}",
                            String::from_utf8_lossy(&c.stderr)
                        ));
                    }
                    let src = std::fs::read_to_string(p).unwrap();
                    let r = src
                        .lines()
                        .find_map(|l| l.strip_prefix("#! respostas:"))
                        .unwrap_or("");
                    let entrada: String = ler_respostas(r)
                        .unwrap()
                        .iter()
                        .map(|(r, t)| format!("{} {t}\n", r.nome()))
                        .collect();
                    let mut filho = Command::new(&bin)
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .spawn()
                        .unwrap();
                    filho
                        .stdin
                        .take()
                        .unwrap()
                        .write_all(entrada.as_bytes())
                        .unwrap();
                    let out = filho.wait_with_output().unwrap();
                    let esperado = std::fs::read_to_string(p.with_extension("esperado")).unwrap();
                    let obtido = String::from_utf8_lossy(&out.stdout);
                    let erro = esperado.lines().any(|l| l.starts_with("FIM erro"));
                    if obtido != esperado {
                        Some(format!(
                            "{nome}: saída diferente\n--- esperado\n{esperado}--- obtido\n{obtido}"
                        ))
                    } else if out.status.success() == erro {
                        Some(format!(
                            "{nome}: código de saída {:?}, esperado {}",
                            out.status.code(),
                            if erro { 1 } else { 0 }
                        ))
                    } else {
                        None
                    }
                })
            })
            .collect();
        tarefas
            .into_iter()
            .filter_map(|t| t.join().unwrap())
            .collect()
    });
    assert!(
        falhas.is_empty(),
        "{} de {} falharam:\n{}",
        falhas.len(),
        executaveis.len(),
        falhas.join("\n")
    );
    println!(
        "{} programas: C++ igual ao interpretador",
        executaveis.len()
    );
}
