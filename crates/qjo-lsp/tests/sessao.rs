//! Conversa com o binário qjo-lsp como um editor faria.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Cliente {
    filho: Child,
    entrada: ChildStdin,
    saida: BufReader<ChildStdout>,
    id: i64,
}

impl Cliente {
    fn novo() -> Self {
        let mut filho = Command::new(env!("CARGO_BIN_EXE_qjo-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let entrada = filho.stdin.take().unwrap();
        let saida = BufReader::new(filho.stdout.take().unwrap());
        Cliente {
            filho,
            entrada,
            saida,
            id: 0,
        }
    }
    fn envia(&mut self, v: Value) {
        let s = v.to_string();
        write!(self.entrada, "Content-Length: {}\r\n\r\n{s}", s.len()).unwrap();
        self.entrada.flush().unwrap();
    }
    fn le(&mut self) -> Value {
        let mut tam = 0;
        loop {
            let mut l = String::new();
            self.saida.read_line(&mut l).unwrap();
            if l == "\r\n" {
                break;
            }
            if let Some(n) = l.strip_prefix("Content-Length: ") {
                tam = n.trim().parse().unwrap();
            }
        }
        let mut b = vec![0; tam];
        self.saida.read_exact(&mut b).unwrap();
        serde_json::from_slice(&b).unwrap()
    }
    fn pede(&mut self, metodo: &str, params: Value) -> Value {
        self.id += 1;
        let id = self.id;
        self.envia(json!({"jsonrpc": "2.0", "id": id, "method": metodo, "params": params}));
        loop {
            let m = self.le();
            if m.get("id") == Some(&json!(id)) {
                assert!(m.get("error").is_none(), "{metodo}: {m}");
                return m["result"].clone();
            }
        }
    }
    fn avisa(&mut self, metodo: &str, params: Value) {
        self.envia(json!({"jsonrpc": "2.0", "method": metodo, "params": params}));
    }
    fn diagnosticos(&mut self) -> Value {
        loop {
            let m = self.le();
            if m["method"] == "textDocument/publishDiagnostics" {
                return m["params"]["diagnostics"].clone();
            }
        }
    }
}

const URI: &str = "file:///tmp/teste.qjo";

#[test]
fn sessao_completa() {
    let mut c = Cliente::novo();
    let init = c.pede(
        "initialize",
        json!({"capabilities": {}, "processId": null, "rootUri": null}),
    );
    assert_eq!(init["serverInfo"]["name"], "qjo-lsp");
    c.avisa("initialized", json!({}));

    // "ação" antes do erro: confere a conversão para UTF-16.
    let src = "# ação\ndata h = data\"08:00:00\";\nobjeto caixa = \"Caixa\";\nrotina(\"R\", h) {\n    INDICAR(caixa);\n    resultado r = AGUARDAR_CONFIRMACAO(30seg);\n}\n";
    c.avisa(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": URI, "languageId": "qjo", "version": 1, "text": src}}),
    );
    let d = c.diagnosticos();
    assert_eq!(d.as_array().unwrap().len(), 1);
    assert_eq!(d[0]["code"], "E_LEX_LITERAL");
    assert_eq!(d[0]["range"]["start"], json!({"line": 5, "character": 39}));

    let acoes = c.pede("textDocument/codeAction", json!({"textDocument": {"uri": URI}, "range": d[0]["range"], "context": {"diagnostics": d}}));
    assert_eq!(acoes[0]["title"], "Escrever 30s");
    assert_eq!(acoes[0]["edit"]["changes"][URI][0]["newText"], "30s");

    let certo = src.replace("30seg", "30s");
    c.avisa(
        "textDocument/didChange",
        json!({"textDocument": {"uri": URI, "version": 2}, "contentChanges": [{"text": certo}]}),
    );
    assert_eq!(c.diagnosticos(), json!([]));

    let h = c.pede(
        "textDocument/hover",
        json!({"textDocument": {"uri": URI}, "position": {"line": 4, "character": 6}}),
    );
    assert!(
        h["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("INDICAR(objeto)")
    );

    let def = c.pede(
        "textDocument/definition",
        json!({"textDocument": {"uri": URI}, "position": {"line": 4, "character": 14}}),
    );
    assert_eq!(def["range"]["start"], json!({"line": 2, "character": 7}));

    let comp = c.pede(
        "textDocument/completion",
        json!({"textDocument": {"uri": URI}, "position": {"line": 5, "character": 4}}),
    );
    let rotulos: Vec<&str> = comp
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap())
        .collect();
    assert!(
        rotulos.contains(&"caixa")
            && rotulos.contains(&"AGUARDAR_CONFIRMACAO")
            && rotulos.contains(&"etapa")
    );

    let ren = c.pede("textDocument/rename", json!({"textDocument": {"uri": URI}, "position": {"line": 2, "character": 8}, "newName": "gaveta"}));
    assert_eq!(ren["changes"][URI].as_array().unwrap().len(), 2);

    let sim = c.pede(
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": URI}}),
    );
    assert_eq!(sim.as_array().unwrap().len(), 3);

    let feio = "data h=data\"08:00:00\";rotina(\"R\",h){ESPERAR(1s);}";
    c.avisa(
        "textDocument/didChange",
        json!({"textDocument": {"uri": URI, "version": 3}, "contentChanges": [{"text": feio}]}),
    );
    c.diagnosticos();
    let f = c.pede(
        "textDocument/formatting",
        json!({"textDocument": {"uri": URI}, "options": {"tabSize": 4, "insertSpaces": true}}),
    );
    assert_eq!(
        f[0]["newText"],
        "data h = data\"08:00:00\";\nrotina(\"R\", h) {\n    ESPERAR(1s);\n}\n"
    );

    c.pede("shutdown", Value::Null);
    c.avisa("exit", Value::Null);
    assert!(c.filho.wait().unwrap().success());
}
