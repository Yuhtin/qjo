//! Núcleo da linguagem Q?JO!: léxico, sintaxe, semântica, interpretador e gerador C++.
//! Sem dependências. A especificação está em `spec/` na raiz do repositório.

pub mod ast;
pub mod cpp;
pub mod diag;
pub mod fmt;
pub mod ide;
pub mod interp;
pub mod lexer;
pub mod parser;
pub mod sema;

pub use diag::{Diagnostico, Edicao, Fase, Linhas, Sugestao};
pub use interp::{
    Dispositivo, Estado, Execucao, Resposta, Resultado, Roteiro, executar, ler_respostas,
};
pub use lexer::{Tok, Token, lex};

/// O resultado de passar um fonte pelas três fases.
pub struct Analise {
    pub tokens: Vec<Token>,
    /// Os erros da primeira fase que falhou (spec/README.md, "Fases").
    pub diagnosticos: Vec<Diagnostico>,
    /// A árvore, mesmo com erros: as partes que a recuperação conseguiu ler.
    pub programa: ast::Programa,
    /// Símbolos e tipos, mesmo com erros (para o editor).
    pub info: sema::Info,
}

impl Analise {
    /// Sem erros em nenhuma fase: o programa pode ser executado e compilado.
    pub fn ok(&self) -> bool {
        self.diagnosticos.is_empty()
    }
}

pub fn analisar(src: &str) -> Analise {
    let lx = lexer::lex(src);
    let mut ps = parser::parse(src, &lx.tokens);
    let ck = sema::checar(src, &mut ps.programa);
    let diagnosticos = if !lx.diagnosticos.is_empty() {
        lx.diagnosticos
    } else if !ps.diagnosticos.is_empty() {
        ps.diagnosticos
    } else {
        ck.diagnosticos
    };
    Analise {
        tokens: lx.tokens,
        diagnosticos,
        programa: ps.programa,
        info: ck.info,
    }
}

/// Executa com respostas fixas; devolve o registro e o resumo. `None` se o programa tem erros.
pub fn rodar(src: &str, respostas: &[(Resultado, i64)]) -> Option<(Vec<String>, Execucao)> {
    let a = analisar(src);
    if !a.ok() {
        return None;
    }
    let mut r = Roteiro::new(respostas.to_vec());
    let e = executar(&a.programa, &a.info, &mut r);
    Some((r.saida, e))
}
