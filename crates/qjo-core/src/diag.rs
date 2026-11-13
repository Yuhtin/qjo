//! Diagnósticos, sugestões e conversão de posições.

/// Uma troca de texto: substitui `ini..fim` (bytes) por `texto`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edicao {
    pub ini: usize,
    pub fim: usize,
    pub texto: String,
}

/// Uma correção evidente para um diagnóstico.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sugestao {
    pub titulo: String,
    pub edicoes: Vec<Edicao>,
}

impl Sugestao {
    pub fn troca(
        titulo: impl Into<String>,
        ini: usize,
        fim: usize,
        texto: impl Into<String>,
    ) -> Self {
        Sugestao {
            titulo: titulo.into(),
            edicoes: vec![Edicao {
                ini,
                fim,
                texto: texto.into(),
            }],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fase {
    Lexico,
    Sintaxe,
    Semantica,
}

impl Fase {
    pub fn nome(self) -> &'static str {
        match self {
            Fase::Lexico => "léxico",
            Fase::Sintaxe => "sintaxe",
            Fase::Semantica => "semântica",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostico {
    pub codigo: &'static str,
    pub mensagem: String,
    /// Trecho em bytes.
    pub ini: usize,
    pub fim: usize,
    pub linha: u32,
    pub coluna: u32,
    pub fase: Fase,
    pub sugestao: Option<Sugestao>,
}

/// Converte deslocamentos em bytes para linha e coluna (pontos de código), e para UTF-16.
pub struct Linhas<'a> {
    src: &'a str,
    inicios: Vec<usize>,
}

impl<'a> Linhas<'a> {
    pub fn new(src: &'a str) -> Self {
        let mut inicios = vec![0];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                inicios.push(i + 1);
            }
        }
        Linhas { src, inicios }
    }

    fn linha_de(&self, off: usize) -> usize {
        match self.inicios.binary_search(&off) {
            Ok(i) => i,
            Err(i) => i - 1,
        }
    }

    /// Linha e coluna começando em 1; a coluna conta pontos de código.
    pub fn posicao(&self, off: usize) -> (u32, u32) {
        let off = off.min(self.src.len());
        let l = self.linha_de(off);
        let col = self.src[self.inicios[l]..off].chars().count();
        (l as u32 + 1, col as u32 + 1)
    }

    /// Linha começando em 0 e coluna em unidades UTF-16 (o que LSP e navegadores usam).
    pub fn utf16(&self, off: usize) -> (u32, u32) {
        let off = off.min(self.src.len());
        let l = self.linha_de(off);
        let col: usize = self.src[self.inicios[l]..off]
            .chars()
            .map(char::len_utf16)
            .sum();
        (l as u32, col as u32)
    }

    /// Inverso de [`Linhas::utf16`].
    pub fn de_utf16(&self, linha: u32, col: u32) -> usize {
        let Some(&ini) = self.inicios.get(linha as usize) else {
            return self.src.len();
        };
        let fim = self
            .inicios
            .get(linha as usize + 1)
            .copied()
            .unwrap_or(self.src.len());
        let mut c = 0u32;
        for (i, ch) in self.src[ini..fim].char_indices() {
            if c >= col || ch == '\n' {
                return ini + i;
            }
            c += ch.len_utf16() as u32;
        }
        fim
    }

    /// Fim da linha que contém `off`, sem a quebra (nem o `\r` de um CRLF).
    pub fn fim_da_linha(&self, off: usize) -> usize {
        let l = self.linha_de(off.min(self.src.len()));
        let ini = self.inicios[l];
        let tem_quebra = l + 1 < self.inicios.len();
        let mut fim = if tem_quebra {
            self.inicios[l + 1] - 1
        } else {
            self.src.len()
        };
        if tem_quebra && fim > ini && self.src.as_bytes()[fim - 1] == b'\r' {
            fim -= 1;
        }
        fim
    }
}

/// Distância de edição, para "você quis dizer".
pub fn distancia(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let custo = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + custo);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// O candidato mais parecido com `nome`, se for parecido o bastante.
pub fn mais_parecido<'b>(nome: &str, candidatos: impl Iterator<Item = &'b str>) -> Option<&'b str> {
    let limite = match nome.chars().count() {
        0..=2 => 1,
        3..=5 => 2,
        _ => 3,
    };
    let baixo = nome.to_lowercase();
    candidatos
        .filter(|c| *c != nome)
        .map(|c| {
            let d = distancia(&baixo, &c.to_lowercase());
            (d, c)
        })
        .filter(|(d, _)| *d <= limite)
        .min_by_key(|(d, c)| (*d, c.len()))
        .map(|(_, c)| c)
}
