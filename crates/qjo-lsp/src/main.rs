//! `qjo-lsp`: servidor de linguagem da Q?JO! pela entrada e saída padrão.
//! Funciona com qualquer editor que fale LSP (VS Code, Cursor, Zed, Neovim, Helix…).

// O Uri do lsp-types tem cache interno; usar como chave é o que a própria API pede.
#![allow(clippy::mutable_key_type)]

use std::collections::HashMap;
use std::error::Error;

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::*;
use qjo_core::ide::{self, EspecieEsboco, EspecieItem};
use qjo_core::{Linhas, analisar, ast::Span, fmt};
use serde_json::{Value, json};

struct Doc {
    texto: String,
}

struct Servidor {
    con: Connection,
    docs: HashMap<String, Doc>,
}

fn faixa(l: &Linhas, ini: usize, fim: usize) -> Range {
    let (a, b) = l.utf16(ini);
    let (c, d) = l.utf16(fim);
    Range::new(Position::new(a, b), Position::new(c, d))
}

fn faixa_span(l: &Linhas, sp: Span) -> Range {
    faixa(l, sp.ini as usize, sp.fim as usize)
}

fn deslocamento(l: &Linhas, p: Position) -> u32 {
    l.de_utf16(p.line, p.character) as u32
}

impl Servidor {
    fn responder<T: serde::Serialize>(&self, id: RequestId, r: T) {
        let _ = self
            .con
            .sender
            .send(Message::Response(Response::new_ok(id, r)));
    }

    fn erro(&self, id: RequestId, msg: String) {
        let _ = self.con.sender.send(Message::Response(Response::new_err(
            id,
            lsp_server::ErrorCode::RequestFailed as i32,
            msg,
        )));
    }

    fn publicar(&self, uri: &Uri) {
        let Some(doc) = self.docs.get(uri.as_str()) else {
            return;
        };
        let a = analisar(&doc.texto);
        let l = Linhas::new(&doc.texto);
        let diags: Vec<Diagnostic> = a
            .diagnosticos
            .iter()
            .map(|d| {
                let data = d.sugestao.as_ref().map(|s| {
                    json!({
                        "titulo": s.titulo,
                        "edicoes": s.edicoes.iter().map(|e| json!({"range": faixa(&l, e.ini, e.fim), "newText": e.texto})).collect::<Vec<_>>()
                    })
                });
                let mensagem = match &d.sugestao {
                    Some(s) if !s.titulo.ends_with('?') => format!("{} — sugestão: {}", d.mensagem, s.titulo),
                    Some(s) => format!("{} — {}", d.mensagem, s.titulo),
                    None => d.mensagem.clone(),
                };
                Diagnostic {
                    range: faixa(&l, d.ini, d.fim),
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String(d.codigo.to_string())),
                    source: Some(format!("Q?JO! {}", d.fase.nome())),
                    message: mensagem,
                    data,
                    ..Default::default()
                }
            })
            .collect();
        let p = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics: diags,
            version: None,
        };
        let _ = self
            .con
            .sender
            .send(Message::Notification(Notification::new(
                "textDocument/publishDiagnostics".into(),
                p,
            )));
    }

    fn notificacao(&mut self, n: Notification) {
        match n.method.as_str() {
            "textDocument/didOpen" => {
                let Ok(p) = serde_json::from_value::<DidOpenTextDocumentParams>(n.params) else {
                    return;
                };
                self.docs.insert(
                    p.text_document.uri.as_str().to_string(),
                    Doc {
                        texto: p.text_document.text,
                    },
                );
                self.publicar(&p.text_document.uri);
            }
            "textDocument/didChange" => {
                let Ok(p) = serde_json::from_value::<DidChangeTextDocumentParams>(n.params) else {
                    return;
                };
                if let Some(c) = p.content_changes.into_iter().last() {
                    self.docs.insert(
                        p.text_document.uri.as_str().to_string(),
                        Doc { texto: c.text },
                    );
                }
                self.publicar(&p.text_document.uri);
            }
            "textDocument/didClose" => {
                let Ok(p) = serde_json::from_value::<DidCloseTextDocumentParams>(n.params) else {
                    return;
                };
                self.docs.remove(p.text_document.uri.as_str());
                let vazio = PublishDiagnosticsParams {
                    uri: p.text_document.uri,
                    diagnostics: vec![],
                    version: None,
                };
                let _ = self
                    .con
                    .sender
                    .send(Message::Notification(Notification::new(
                        "textDocument/publishDiagnostics".into(),
                        vazio,
                    )));
            }
            _ => {}
        }
    }

    fn texto(&self, uri: &Uri) -> Option<&str> {
        self.docs.get(uri.as_str()).map(|d| d.texto.as_str())
    }

    fn pedido(&mut self, r: Request) {
        let id = r.id.clone();
        let res: Result<Value, String> = self.tratar(&r.method, r.params);
        match res {
            Ok(v) => self.responder(id, v),
            Err(e) => self.erro(id, e),
        }
    }

    fn tratar(&self, metodo: &str, params: Value) -> Result<Value, String> {
        macro_rules! p {
            ($t:ty) => {
                serde_json::from_value::<$t>(params).map_err(|e| e.to_string())?
            };
        }
        let nada = Ok(Value::Null);
        match metodo {
            "textDocument/hover" => {
                let p = p!(HoverParams);
                let pos = p.text_document_position_params;
                let Some(src) = self.texto(&pos.text_document.uri) else {
                    return nada;
                };
                let (a, l) = (analisar(src), Linhas::new(src));
                let Some((sp, md)) = ide::dica(&a, src, deslocamento(&l, pos.position)) else {
                    return nada;
                };
                let h = Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: md,
                    }),
                    range: Some(faixa_span(&l, sp)),
                };
                Ok(json!(h))
            }
            "textDocument/completion" => {
                let p = p!(CompletionParams);
                let pos = p.text_document_position;
                let Some(src) = self.texto(&pos.text_document.uri) else {
                    return nada;
                };
                let (a, l) = (analisar(src), Linhas::new(src));
                let itens: Vec<CompletionItem> = ide::completar(&a, deslocamento(&l, pos.position))
                    .into_iter()
                    .map(|i| CompletionItem {
                        label: i.rotulo.clone(),
                        kind: Some(match i.especie {
                            EspecieItem::Palavra => CompletionItemKind::KEYWORD,
                            EspecieItem::Tipo => CompletionItemKind::TYPE_PARAMETER,
                            EspecieItem::Primitiva | EspecieItem::Funcao => {
                                CompletionItemKind::FUNCTION
                            }
                            EspecieItem::Variavel => CompletionItemKind::VARIABLE,
                            EspecieItem::Constante => CompletionItemKind::CONSTANT,
                            EspecieItem::ValorEnum => CompletionItemKind::ENUM_MEMBER,
                            EspecieItem::Enum => CompletionItemKind::ENUM,
                            EspecieItem::Trecho => CompletionItemKind::SNIPPET,
                        }),
                        detail: Some(i.detalhe),
                        insert_text_format: i.snippet.as_ref().map(|_| InsertTextFormat::SNIPPET),
                        insert_text: i.snippet,
                        ..Default::default()
                    })
                    .collect();
                Ok(json!(itens))
            }
            "textDocument/definition" => {
                let p = p!(GotoDefinitionParams);
                let pos = p.text_document_position_params;
                let Some(src) = self.texto(&pos.text_document.uri) else {
                    return nada;
                };
                let (a, l) = (analisar(src), Linhas::new(src));
                let Some(d) = ide::definicao(&a, deslocamento(&l, pos.position)) else {
                    return nada;
                };
                Ok(json!(Location::new(
                    pos.text_document.uri,
                    faixa_span(&l, d)
                )))
            }
            "textDocument/references" => {
                let p = p!(ReferenceParams);
                let pos = p.text_document_position;
                let Some(src) = self.texto(&pos.text_document.uri) else {
                    return nada;
                };
                let (a, l) = (analisar(src), Linhas::new(src));
                let locs: Vec<Location> = ide::referencias(
                    &a,
                    deslocamento(&l, pos.position),
                    p.context.include_declaration,
                )
                .into_iter()
                .map(|sp| Location::new(pos.text_document.uri.clone(), faixa_span(&l, sp)))
                .collect();
                Ok(json!(locs))
            }
            "textDocument/prepareRename" => {
                let p = p!(TextDocumentPositionParams);
                let Some(src) = self.texto(&p.text_document.uri) else {
                    return nada;
                };
                let (a, l) = (analisar(src), Linhas::new(src));
                match ide::preparar_renomear(&a, deslocamento(&l, p.position)) {
                    Some((sp, nome)) => {
                        Ok(json!({"range": faixa_span(&l, sp), "placeholder": nome}))
                    }
                    None => nada,
                }
            }
            "textDocument/rename" => {
                let p = p!(RenameParams);
                let pos = p.text_document_position;
                let Some(src) = self.texto(&pos.text_document.uri) else {
                    return nada;
                };
                let (a, l) = (analisar(src), Linhas::new(src));
                let ed = ide::renomear(&a, deslocamento(&l, pos.position), &p.new_name)?;
                let edits: Vec<TextEdit> = ed
                    .into_iter()
                    .map(|e| TextEdit::new(faixa(&l, e.ini, e.fim), e.texto))
                    .collect();
                let mut changes = HashMap::new();
                changes.insert(pos.text_document.uri, edits);
                Ok(json!(WorkspaceEdit {
                    changes: Some(changes),
                    ..Default::default()
                }))
            }
            "textDocument/documentSymbol" => {
                let p = p!(DocumentSymbolParams);
                let Some(src) = self.texto(&p.text_document.uri) else {
                    return nada;
                };
                let (a, l) = (analisar(src), Linhas::new(src));
                fn conv(n: ide::NoEsboco, l: &Linhas) -> DocumentSymbol {
                    #[allow(deprecated)]
                    DocumentSymbol {
                        name: if n.nome.trim().is_empty() {
                            "(sem nome)".into()
                        } else {
                            n.nome
                        },
                        detail: None,
                        kind: match n.especie {
                            EspecieEsboco::Rotina => SymbolKind::MODULE,
                            EspecieEsboco::Etapa => SymbolKind::EVENT,
                            EspecieEsboco::Funcao => SymbolKind::FUNCTION,
                            EspecieEsboco::Enum => SymbolKind::ENUM,
                            EspecieEsboco::Recurso => SymbolKind::CONSTANT,
                            EspecieEsboco::Horario => SymbolKind::CONSTANT,
                        },
                        tags: None,
                        deprecated: None,
                        range: faixa_span(l, n.span),
                        selection_range: faixa_span(l, n.selecao),
                        children: Some(n.filhos.into_iter().map(|f| conv(f, l)).collect()),
                    }
                }
                let v: Vec<DocumentSymbol> = ide::esboco(&a.programa)
                    .into_iter()
                    .map(|n| conv(n, &l))
                    .collect();
                Ok(json!(v))
            }
            "textDocument/formatting" => {
                let p = p!(DocumentFormattingParams);
                let Some(src) = self.texto(&p.text_document.uri) else {
                    return nada;
                };
                let Some(novo) = fmt::formatar(src) else {
                    return nada;
                };
                if novo == src {
                    return Ok(json!([]));
                }
                let l = Linhas::new(src);
                Ok(json!([TextEdit::new(faixa(&l, 0, src.len()), novo)]))
            }
            "textDocument/codeAction" => {
                let p = p!(CodeActionParams);
                let acoes: Vec<CodeAction> = p
                    .context
                    .diagnostics
                    .iter()
                    .filter_map(|d| {
                        let data = d.data.as_ref()?;
                        let titulo = data.get("titulo")?.as_str()?.to_string();
                        let edits: Vec<TextEdit> =
                            serde_json::from_value(data.get("edicoes")?.clone()).ok()?;
                        let mut changes = HashMap::new();
                        changes.insert(p.text_document.uri.clone(), edits);
                        Some(CodeAction {
                            title: titulo,
                            kind: Some(CodeActionKind::QUICKFIX),
                            diagnostics: Some(vec![d.clone()]),
                            edit: Some(WorkspaceEdit {
                                changes: Some(changes),
                                ..Default::default()
                            }),
                            is_preferred: Some(true),
                            ..Default::default()
                        })
                    })
                    .collect();
                Ok(json!(acoes))
            }
            _ => Err(format!("método não suportado: {metodo}")),
        }
    }
}

fn capacidades() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        completion_provider: Some(CompletionOptions::default()),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        document_symbol_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
            ..Default::default()
        })),
        ..Default::default()
    }
}

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("qjo-lsp {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let (con, io) = Connection::stdio();
    let caps = serde_json::to_value(capacidades())?;
    let init = json!({ "capabilities": caps, "serverInfo": { "name": "qjo-lsp", "version": env!("CARGO_PKG_VERSION") } });
    let id_e_params = con.initialize_start()?;
    con.initialize_finish(id_e_params.0, init)?;
    let mut s = Servidor {
        con,
        docs: HashMap::new(),
    };
    while let Ok(msg) = s.con.receiver.recv() {
        match msg {
            Message::Request(r) => {
                if s.con.handle_shutdown(&r)? {
                    break;
                }
                s.pedido(r);
            }
            Message::Notification(n) => s.notificacao(n),
            Message::Response(_) => {}
        }
    }
    drop(s);
    io.join()?;
    Ok(())
}
