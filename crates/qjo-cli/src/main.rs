//! `qjo`: linha de comando da linguagem Q?JO!.

use std::io::{BufRead, IsTerminal, Read, Write};
use std::process::ExitCode;

use qjo_core::interp::{Dispositivo, Resposta, Resultado};
use qjo_core::{Analise, Diagnostico, Linhas, analisar, cpp, executar, fmt, ler_respostas};

const AJUDA: &str = "qjo — ferramentas da linguagem Q?JO!

Uso:
  qjo check <arquivo.qjo>...            confere léxico, sintaxe e semântica
  qjo run <arquivo.qjo> [--respostas R]  executa a rotina no simulador
  qjo build <arquivo.qjo> [-o saida.cpp] gera C++17 (com --exe, compila e roda)
  qjo tokens <arquivo.qjo> [--json]      lista os tokens
  qjo fmt <arquivo.qjo>... [--check]     formata no lugar (--stdout imprime)

Respostas: \"CONFIRMADO 3, AJUDA 5, TEMPO_ESGOTADO\" (ou c/a/t). Sem --respostas, o run
pergunta no terminal; com a entrada redirecionada, lê uma resposta por linha.

Use - como arquivo para ler da entrada padrão.
Saída: 0 sem erros, 1 com erros no programa, 2 uso incorreto ou falha de leitura.";

struct Cores {
    on: bool,
}

impl Cores {
    fn p(&self, cod: &str, s: &str) -> String {
        if self.on {
            format!("\x1b[{cod}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
}

fn ler(caminho: &str) -> Result<String, String> {
    if caminho == "-" {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| format!("não consegui ler a entrada padrão: {e}"))?;
        return Ok(s);
    }
    let bytes = std::fs::read(caminho).map_err(|e| format!("não consegui ler {caminho}: {e}"))?;
    String::from_utf8(bytes).map_err(|_| format!("{caminho} não está em UTF-8"))
}

fn mostrar(arquivo: &str, src: &str, d: &Diagnostico, c: &Cores, curto: bool) {
    if curto {
        eprintln!(
            "{arquivo}:{}:{}: {}: {}",
            d.linha, d.coluna, d.codigo, d.mensagem
        );
        return;
    }
    let li = Linhas::new(src);
    let ini = d.ini.min(src.len());
    let ini_linha = src[..ini].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let fim_linha = li.fim_da_linha(d.ini);
    let texto = &src[ini_linha..fim_linha.max(ini_linha)];
    let largura = src[ini..d.fim.min(fim_linha).max(ini)]
        .chars()
        .count()
        .max(1);
    let num = d.linha.to_string();
    let pad = " ".repeat(num.len());
    eprintln!(
        "{}{} {}",
        c.p("1;31", &format!("erro[{}]", d.codigo)),
        c.p("1", ":"),
        c.p("1", &d.mensagem)
    );
    eprintln!(
        "{pad}{} {arquivo}:{}:{}",
        c.p("1;34", "-->"),
        d.linha,
        d.coluna
    );
    eprintln!("{pad} {}", c.p("1;34", "|"));
    eprintln!(
        "{} {} {}",
        c.p("1;34", &num),
        c.p("1;34", "|"),
        texto.replace('\t', " ")
    );
    eprintln!(
        "{pad} {} {}{}",
        c.p("1;34", "|"),
        " ".repeat(d.coluna as usize - 1),
        c.p("1;31", &"^".repeat(largura))
    );
    if let Some(s) = &d.sugestao {
        eprintln!(
            "{pad} {} {}",
            c.p("1;34", "="),
            c.p("1;32", &format!("sugestão: {}", s.titulo))
        );
    }
    eprintln!();
}

fn relatar(arquivo: &str, src: &str, a: &Analise, c: &Cores, curto: bool) -> bool {
    for d in &a.diagnosticos {
        mostrar(arquivo, src, d, c, curto);
    }
    if !a.diagnosticos.is_empty() && !curto {
        let n = a.diagnosticos.len();
        eprintln!(
            "{}",
            c.p(
                "1;31",
                &format!(
                    "{n} erro{} de {} em {arquivo}",
                    if n == 1 { "" } else { "s" },
                    a.diagnosticos[0].fase.nome()
                )
            )
        );
    }
    a.ok()
}

struct Terminal {
    cores: Cores,
}

impl Dispositivo for Terminal {
    fn registro(&mut self, linha: &str) {
        println!("{}", colorir(linha, &self.cores));
    }
    fn aguardar(&mut self, limite: i64) -> Resposta {
        let stdin = std::io::stdin();
        loop {
            eprint!(
                "{} aguardando confirmação (limite {limite}s) — c, a ou t, e os segundos [c 0]: ",
                self.cores.p("1;36", "?")
            );
            let _ = std::io::stderr().flush();
            let mut l = String::new();
            if stdin.lock().read_line(&mut l).unwrap_or(0) == 0 {
                eprintln!();
                return Resposta::Nenhuma;
            }
            let l = if l.trim().is_empty() {
                "c".to_string()
            } else {
                l
            };
            match ler_respostas(&l) {
                Ok(v) if v.len() == 1 => return Resposta::Dada(v[0].0, v[0].1),
                _ => eprintln!(
                    "  use c (CONFIRMADO), a (AJUDA) ou t (TEMPO_ESGOTADO), por exemplo: a 5"
                ),
            }
        }
    }
}

struct Lista {
    respostas: Vec<(Resultado, i64)>,
    k: usize,
    cores: Cores,
}

impl Dispositivo for Lista {
    fn registro(&mut self, linha: &str) {
        println!("{}", colorir(linha, &self.cores));
    }
    fn aguardar(&mut self, _: i64) -> Resposta {
        self.k += 1;
        self.respostas
            .get(self.k - 1)
            .map(|&(r, t)| Resposta::Dada(r, t))
            .unwrap_or(Resposta::Nenhuma)
    }
}

fn colorir(l: &str, c: &Cores) -> String {
    let cod = if l.starts_with("ROTINA ") {
        "1;35"
    } else if l.starts_with("ETAPA ") {
        "1"
    } else if l.starts_with("FIM concluida") {
        "1;32"
    } else if l.starts_with("FIM ") {
        "1;31"
    } else if l.starts_with("ALERTAR") {
        "33"
    } else if l.starts_with("AGUARDAR") {
        "36"
    } else if l.ends_with("(fim da etapa)") || l.ends_with("(fim da rotina)") {
        "2"
    } else {
        return l.to_string();
    };
    c.p(cod, l)
}

fn json(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 32 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn principal(args: Vec<String>) -> Result<bool, String> {
    let cores = Cores {
        on: std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
    };
    let cores_out = Cores {
        on: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
    };
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let mut arquivos = vec![];
    let mut flags = std::collections::HashMap::<&str, Option<String>>::new();
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--respostas" | "-r" | "-o" | "--saida" | "--formato" => {
                let v = it.next().ok_or(format!("{a} precisa de um valor"))?;
                let chave = match a.as_str() {
                    "-r" => "--respostas",
                    "--saida" => "-o",
                    x => x,
                };
                flags.insert(chave, Some(v.clone()));
            }
            "--json" | "--check" | "--stdout" | "--exe" | "--curto" => {
                flags.insert(a.as_str(), None);
            }
            x if x.starts_with("--") => return Err(format!("opção desconhecida: {x}")),
            x => arquivos.push(x.to_string()),
        }
    }
    let curto = flags.contains_key("--curto")
        || flags
            .get("--formato")
            .map(|v| v.as_deref() == Some("curto"))
            .unwrap_or(false);
    let um = |arquivos: &Vec<String>| -> Result<String, String> {
        match arquivos.as_slice() {
            [a] => Ok(a.clone()),
            [] => Err(format!("qjo {cmd} precisa de um arquivo .qjo")),
            _ => Err(format!("qjo {cmd} recebe um arquivo só")),
        }
    };

    match cmd {
        "check" => {
            if arquivos.is_empty() {
                return Err("qjo check precisa de ao menos um arquivo .qjo".into());
            }
            let mut ok = true;
            for f in &arquivos {
                let src = ler(f)?;
                let a = analisar(&src);
                ok &= relatar(f, &src, &a, &cores, curto);
            }
            if ok && !curto {
                eprintln!(
                    "{}",
                    cores.p("1;32", &format!("ok: {} sem erros", arquivos.join(", ")))
                );
            }
            Ok(ok)
        }
        "run" => {
            let f = um(&arquivos)?;
            let src = ler(&f)?;
            let a = analisar(&src);
            if !relatar(&f, &src, &a, &cores, curto) {
                return Ok(false);
            }
            let exec = if let Some(Some(r)) = flags.get("--respostas") {
                let respostas = ler_respostas(r).map_err(|p| format!("resposta inválida: {p}"))?;
                executar(
                    &a.programa,
                    &a.info,
                    &mut Lista {
                        respostas,
                        k: 0,
                        cores: cores_out,
                    },
                )
            } else if f != "-" && std::io::stdin().is_terminal() {
                executar(&a.programa, &a.info, &mut Terminal { cores: cores_out })
            } else {
                let mut s = String::new();
                if f != "-" {
                    std::io::stdin()
                        .read_to_string(&mut s)
                        .map_err(|e| e.to_string())?;
                }
                let respostas = ler_respostas(&s).map_err(|p| format!("resposta inválida: {p}"))?;
                executar(
                    &a.programa,
                    &a.info,
                    &mut Lista {
                        respostas,
                        k: 0,
                        cores: cores_out,
                    },
                )
            };
            Ok(exec
                .rotinas
                .iter()
                .all(|(_, e)| !matches!(e, qjo_core::Estado::Erro(_))))
        }
        "build" => {
            let f = um(&arquivos)?;
            let src = ler(&f)?;
            let a = analisar(&src);
            if !relatar(&f, &src, &a, &cores, curto) {
                return Ok(false);
            }
            let nome = std::path::Path::new(&f)
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("rotina.qjo")
                .to_string();
            let codigo = cpp::gerar(&a.programa, &a.info, &nome);
            let saida = match flags.get("-o") {
                Some(Some(o)) => o.clone(),
                _ if f == "-" => "-".into(),
                _ => f.trim_end_matches(".qjo").to_string() + ".cpp",
            };
            if saida == "-" {
                print!("{codigo}");
                return Ok(true);
            }
            std::fs::write(&saida, &codigo)
                .map_err(|e| format!("não consegui escrever {saida}: {e}"))?;
            eprintln!("{}", cores.p("1;32", &format!("C++ gerado em {saida}")));
            if flags.contains_key("--exe") {
                let bin = saida.trim_end_matches(".cpp").to_string();
                let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
                let st = std::process::Command::new(&cxx)
                    .args(["-std=c++17", "-O2", "-o", &bin, &saida])
                    .status()
                    .map_err(|e| format!("não consegui rodar {cxx}: {e}"))?;
                if !st.success() {
                    return Err(format!("{cxx} falhou ao compilar {saida}"));
                }
                eprintln!("{}", cores.p("1;32", &format!("binário em {bin}")));
            }
            Ok(true)
        }
        "tokens" => {
            let f = um(&arquivos)?;
            let src = ler(&f)?;
            let lx = qjo_core::lex(&src);
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            for t in &lx.tokens {
                let l = t.lexema(&src);
                let _ = if flags.contains_key("--json") {
                    writeln!(
                        out,
                        "{{\"arquivo\":{},\"linha\":{},\"coluna\":{},\"token\":\"{}\",\"lexema\":{}}}",
                        json(&f),
                        t.linha,
                        t.coluna,
                        t.tipo.nome(),
                        json(l)
                    )
                } else {
                    writeln!(out, "{}:{} {} {}", t.linha, t.coluna, t.tipo.nome(), l)
                };
            }
            drop(out);
            for d in &lx.diagnosticos {
                eprintln!("{f}:{}:{}: {}: {}", d.linha, d.coluna, d.codigo, d.mensagem);
            }
            Ok(lx.diagnosticos.is_empty())
        }
        "fmt" => {
            if arquivos.is_empty() {
                return Err("qjo fmt precisa de ao menos um arquivo .qjo".into());
            }
            let mut ok = true;
            for f in &arquivos {
                let src = ler(f)?;
                let Some(novo) = fmt::formatar(&src) else {
                    eprintln!("{f}: não formatei porque há erro léxico (rode qjo check)");
                    ok = false;
                    continue;
                };
                if flags.contains_key("--stdout") || f == "-" {
                    print!("{novo}");
                } else if flags.contains_key("--check") {
                    if novo != src {
                        eprintln!("{f}: precisa de formatação");
                        ok = false;
                    }
                } else if novo != src {
                    std::fs::write(f, novo)
                        .map_err(|e| format!("não consegui escrever {f}: {e}"))?;
                    eprintln!("formatado: {f}");
                }
            }
            Ok(ok)
        }
        "" | "-h" | "--help" | "ajuda" | "help" => {
            println!("{AJUDA}");
            Ok(true)
        }
        "-V" | "--version" | "versao" => {
            println!("qjo {}", env!("CARGO_PKG_VERSION"));
            Ok(true)
        }
        x => Err(format!("comando desconhecido: {x} (veja qjo --help)")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match principal(args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("qjo: {e}");
            ExitCode::from(2)
        }
    }
}
