//! Mede análise (léxico + sintaxe + semântica) e execução de um arquivo.
//! cargo run --release -p qjo-core --example bench -- arquivo.qjo [rodadas]

use std::time::Instant;

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let src = std::fs::read_to_string(&args[1]).unwrap();
    let rodadas: usize = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(30);
    let mut analise = vec![];
    let mut execucao = vec![];
    let (mut fl, mut fp, mut fs) = (vec![], vec![], vec![]);
    for _ in 0..rodadas {
        let t = Instant::now();
        let a = qjo_core::analisar(&src);
        analise.push(t.elapsed().as_secs_f64() * 1000.0);
        assert!(a.ok(), "{:?}", a.diagnosticos.first());
        let t = Instant::now();
        let lx = qjo_core::lexer::lex(&src);
        fl.push(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        let mut ps = qjo_core::parser::parse(&src, &lx.tokens);
        fp.push(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        qjo_core::sema::checar(&src, &mut ps.programa);
        fs.push(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        let mut r = qjo_core::Roteiro::new(vec![]);
        qjo_core::executar(&a.programa, &a.info, &mut r);
        execucao.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    println!(
        "{{\"analise_ms\":{:.3},\"lexico_ms\":{:.3},\"sintaxe_ms\":{:.3},\"semantica_ms\":{:.3},\"execucao_ms\":{:.3}}}",
        mediana(analise),
        mediana(fl),
        mediana(fp),
        mediana(fs),
        mediana(execucao)
    );
}
