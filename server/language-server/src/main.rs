use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::{Language, Node, Parser, TreeCursor};
use treesitter_ts::tree_sitter_typescript;

#[derive(Serialize, Deserialize, Debug)]
struct PostfixTemplate {
    snippetKey: String,
    functionName: String,
}

struct Backend {
    client: Client,
    document_map: Arc<Mutex<HashMap<String, TextDocumentItem>>>,
    parser: Arc<Mutex<Parser>>,
    postfix_template_list: Arc<Mutex<Vec<PostfixTemplate>>>,
}
impl Backend {
    async fn reset_templates(&self) {
        let configuration = self
            .client
            .configuration(vec![ConfigurationItem {
                scope_uri: None,
                section: Some("tjs-postfix.templateMapList".into()),
            }])
            .await;
        if let Ok(mut configuration) = configuration {
            if let Ok(configuration) = serde_json::from_value::<Vec<PostfixTemplate>>(
                configuration.first_mut().unwrap().take(),
            ) {
                match self.postfix_template_list.lock() {
                    Ok(mut list) => {
                        list.clear();
                        list.extend(configuration);
                    }
                    Err(_) => {}
                }
            }
        }
    }
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
        self.reset_templates().await;
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
        self.reset_templates().await;
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
                            let mut template_item_list =
                                if let Ok(template_list) = self.postfix_template_list.lock() {
                                    template_list
                                        .iter()
                                        .map(|template_item| {
                                            let mut item = CompletionItem::new_simple(
                                                template_item.snippetKey.clone(),
                                                template_item.functionName.clone(),
                                            );
                                            item.kind = Some(CompletionItemKind::Snippet);
                                            let replace_string = format!(
                                                "{}({})",
                                                &template_item.functionName,
                                                &document.text[node.byte_range()]
                                            );
                                            item.documentation =
                                                Some(Documentation::String(replace_string.clone()));
                                            let replace_range = Range::new(
                                                Position::new(
                                                    node.start_position().row as u64,
                                                    node.start_position().column as u64,
                                                ),
                                                Position::new(dot.line, dot.character),
                                            );

                                            item.insert_text = Some(replace_string);
                                            item.additional_text_edits =
                                                Some(vec![TextEdit::new(replace_range, "".into())]);
                                            item
                                        })
                                        .collect()
                                } else {
                                    vec![]
                                };
                            template_item_list.push(CompletionItem::new_simple(
                                format!("{:?}", duration),
                                format!("{:?}: {:?}", node, Range::default()),
                            ));
                            return Ok(Some(CompletionResponse::Array(template_item_list)));
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
        let postfix_template_list = Arc::new(Mutex::new(vec![]));
        Backend {
            client,
            document_map,
            parser,
            postfix_template_list,
        }
    });
    Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service)
        .await;
}
