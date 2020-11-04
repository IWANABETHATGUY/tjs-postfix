use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use log::info;
use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::{Language, Node, Parser, TreeCursor};
use treesitter_ts::tree_sitter_typescript;
struct Backend {
    client: Client,
    document_map: Arc<Mutex<HashMap<String, TextDocumentItem>>>,
    parser: Arc<Mutex<Parser>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::Full,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    work_done_progress_options: Default::default(),
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                workspace: Some(WorkspaceCapability {
                    workspace_folders: Some(WorkspaceFolderCapability {
                        supported: Some(true),
                        change_notifications: Some(
                            WorkspaceFolderCapabilityChangeNotifications::Bool(true),
                        ),
                    }),
                }),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::Info, "initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        self.client
            .log_message(MessageType::Info, "workspace folders changed!")
            .await;
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::Info, "configuration changed!")
            .await;
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        self.client
            .log_message(MessageType::Info, "watched files have changed!")
            .await;
    }

    async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
        self.client
            .log_message(MessageType::Info, "command executed!")
            .await;

        match self.client.apply_edit(WorkspaceEdit::default()).await {
            Ok(res) if res.applied => self.client.log_message(MessageType::Info, "applied").await,
            Ok(_) => self.client.log_message(MessageType::Info, "rejected").await,
            Err(err) => self.client.log_message(MessageType::Error, err).await,
        }

        Ok(None)
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document_uri = params.text_document.uri.clone();
        self.document_map
            .lock()
            .unwrap()
            .insert(document_uri.to_string(), params.text_document);
        self.client
            .log_message(MessageType::Info, document_uri.to_string())
            .await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        if let Some(document) = self
            .document_map
            .lock()
            .unwrap()
            .get_mut(&params.text_document.uri.to_string())
        {
            if let Some(content) = params.content_changes.first_mut().take() {
                document.text = content.text.clone();
            }
        }
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::Info, "file saved!")
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut map = self.document_map.lock().unwrap();
        map.remove(&params.text_document.uri.to_string());
    }
    // const range = document.getWordRangeAtPosition(position, /[^\s]\.[a-zA-Z]*/);
    //     if (!range) {
    //       return [];
    //     }
    //     let curNode = this.tree.rootNode.namedDescendantForPosition({
    //       column: beforeDot.character,
    //       row: beforeDot.line,
    //     });
    //     let endIndex = curNode.endIndex;
    //     while (true) {
    //       if (curNode.parent && curNode.parent.endIndex === endIndex && curNode.type !== "ERROR") {
    //         curNode = curNode.parent;
    //       } else {
    //         break;
    //       }
    //     }
    //     // console.log(curNode.type);
    //     return this.templateList.map(template => {
    //       const item = new CompletionItem(template.snippetKey);
    //       item.kind = CompletionItemKind.Snippet;
    //       item.insertText = "";
    //       item.keepWhitespace = true;
    //       const replaceString = `${template.functionName}(${curNode.text})`;
    //       item.documentation = replaceString;
    //       const replaceRange = new Range(
    //         curNode.startPosition.row,
    //         curNode.startPosition.column,
    //         range.end.line,
    //         range.end.character
    //       );
    //       // console.log(curNode.text);
    //       item.additionalTextEdits = [TextEdit.replace(replaceRange, replaceString)];
    //       return item;
    //     });
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        if let Some(context) = params.context {
            if context.trigger_character.is_none() || context.trigger_character.unwrap() != "." {
                return Ok(None);
            }
            if let Some(document) = self
                .document_map
                .lock()
                .unwrap()
                .get(&params.text_document_position.text_document.uri.to_string())
            {
                match self.parser.lock() {
                    Ok(mut parser) => {
                        let start = Instant::now();
                        let tree = parser.parse(&document.text, None).unwrap();
                        let duration = start.elapsed();

                        let root = tree.root_node();
                        let dot = params.text_document_position.position;
                        let before_dot = Position::new(dot.line, dot.character - 2);

                        let node = root.named_descendant_for_point_range(
                            tree_sitter::Point::new(
                                before_dot.line as usize,
                                before_dot.character as usize,
                            ),
                            tree_sitter::Point::new(
                                before_dot.line as usize,
                                before_dot.character as usize,
                            ),
                        );

                        if let Some(mut node) = node {
                            let end_index = node.end_byte();
                            while let Some(parent) = node.parent() {
                                if !node.is_error() && parent.end_byte() == end_index {
                                    node = parent;
                                } else {
                                    break;
                                }
                            }
                            //       const item = new CompletionItem(template.snippetKey);
                            //       item.kind = CompletionItemKind.Snippet;
                            //       item.insertText = "";
                            //       item.keepWhitespace = true;
                            //       const replaceString = `${template.functionName}(${curNode.text})`;
                            //       item.documentation = replaceString;
                            //       const replaceRange = new Range(
                            //         curNode.startPosition.row,
                            //         curNode.startPosition.column,
                            //         range.end.line,
                            //         range.end.character
                            //       );
                            //       // console.log(curNode.text);
                            //       item.additionalTextEdits = [TextEdit.replace(replaceRange, replaceString)];
                            //       return item;
                            let mut item =
                                CompletionItem::new_simple("log".into(), "log something".into());
                            item.kind = Some(CompletionItemKind::Snippet);
                            item.insert_text = Some(" ".into());
                            let replace_string =
                                format!("{}({})", "console.log", &document.text[node.byte_range()]);
                            item.documentation =
                                Some(Documentation::String(replace_string.clone()));
                            let replace_range = Range::new(
                                Position::new(
                                    node.start_position().row as u64,
                                    node.start_position().column as u64,
                                ),
                                Position::new(dot.line, dot.character),
                            );

                            item.additional_text_edits =
                                Some(vec![TextEdit::new(replace_range, replace_string)]);
                            return Ok(Some(CompletionResponse::Array(vec![
                                item,
                                CompletionItem::new_simple(
                                    format!("{:?}", duration),
                                    format!("{:?}: {:?}", node, replace_range),
                                ),
                            ])));
                        }
                    }
                    Err(_) => {}
                };
            }
        }
        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, messages) = LspService::new(|client| {
        let language = unsafe { tree_sitter_typescript() };
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(language).unwrap();
        let parser = Arc::new(Mutex::new(parser));
        let document_map = Arc::new(Mutex::new(HashMap::new()));
        Backend {
            client,
            document_map,
            parser,
        }
    });
    Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service)
        .await;
}
