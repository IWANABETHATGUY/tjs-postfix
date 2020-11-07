use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use log::debug;
use lsp_text_document::FullTextDocument;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tree_sitter::Parser;

#[derive(Serialize, Deserialize, Debug)]
pub struct PostfixTemplate {
    snippetKey: String,
    functionName: String,
}

pub struct SnippetCompletionItem {
    label: String,
    detail: String,
    replace_string_generator: Box<dyn Fn(String) -> String>,
}
pub struct Backend {
    client: Client,
    document_map: Arc<Mutex<HashMap<String, FullTextDocument>>>,
    parser: Arc<Mutex<Parser>>,
    postfix_template_list: Arc<Mutex<Vec<PostfixTemplate>>>,
}
impl Backend {
    pub fn new(
        client: Client,
        document_map: Arc<Mutex<HashMap<String, FullTextDocument>>>,
        parser: Arc<Mutex<Parser>>,
        postfix_template_list: Arc<Mutex<Vec<PostfixTemplate>>>,
    ) -> Self {
        Self {
            client,
            document_map,
            parser,
            postfix_template_list,
        }
    }

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

    fn get_template_completion_item_list(
        &self,
        source_code: String,
        replace_range: &Range,
    ) -> Vec<CompletionItem> {
        if let Ok(template_list) = self.postfix_template_list.lock() {
            template_list
                .iter()
                .map(|template_item| {
                    let mut item = CompletionItem::new_simple(
                        template_item.snippetKey.clone(),
                        template_item.functionName.clone(),
                    );
                    item.kind = Some(CompletionItemKind::Snippet);
                    let replace_string =
                        format!("{}({})", &template_item.functionName, source_code);
                    item.documentation = Some(Documentation::String(replace_string.clone()));
                    item.insert_text = Some(replace_string);
                    item.additional_text_edits =
                        Some(vec![TextEdit::new(replace_range.clone(), "".into())]);
                    item
                })
                .collect()
        } else {
            vec![]
        }
    }

    fn get_snippet_completion_item_list(
        &self,
        source_code: String,
        replace_range: &Range,
    ) -> Vec<CompletionItem> {
        let snippet_list = vec![
            SnippetCompletionItem {
                label: String::from("not"),
                detail: String::from("revert a variable or expression"),
                replace_string_generator: Box::new(|name| format!("!{}", name)),
            },
            SnippetCompletionItem {
                label: String::from("if"),
                detail: String::from("if (expr)"),
                replace_string_generator: Box::new(|name| {
                    format!(
                        r#"if ({}) {{
    ${{0}}
}}"#,
                        name
                    )
                }),
            },
            SnippetCompletionItem {
                label: String::from("var"),
                detail: String::from("var name = expr"),
                replace_string_generator: Box::new(|name| format!("var ${{0}} = {}", name)),
            },
            SnippetCompletionItem {
                label: String::from("let"),
                detail: String::from("let name = expr"),
                replace_string_generator: Box::new(|name| format!("let ${{0}} = {}", name)),
            },
            SnippetCompletionItem {
                label: String::from("const"),
                detail: String::from("const name = expr"),
                replace_string_generator: Box::new(|name| format!("const ${{0}} = {}", name)),
            },
            SnippetCompletionItem {
                label: String::from("cast"),
                detail: String::from("(<name>expr)"),
                replace_string_generator: Box::new(|name| format!("(<${{0}}>{})", name)),
            },
            SnippetCompletionItem {
                label: String::from("as"),
                detail: String::from("(expr as name)"),
                replace_string_generator: Box::new(|name| format!("({} as ${{0}})", name)),
            },
            SnippetCompletionItem {
                label: String::from("new"),
                detail: String::from("new expr()"),
                replace_string_generator: Box::new(|name| format!("new {}()", name)),
            },
            SnippetCompletionItem {
                label: String::from("return"),
                detail: String::from("return expr"),
                replace_string_generator: Box::new(|name| format!("return {}", name)),
            },
            // foreach
            SnippetCompletionItem {
                label: String::from("for"),
                detail: String::from("forloop"),
                replace_string_generator: Box::new(|name| {
                    format!(
                        r#"for (let ${{1:i}} = 0, len = {}.length; ${{1:i}} < len; ${{1:i}}++) {{
  ${{0}}
}}"#,
                        name
                    )
                }),
            },
            SnippetCompletionItem {
                label: String::from("forof"),
                detail: String::from("forof"),
                replace_string_generator: Box::new(|name| {
                    format!(
                        r#"for (let ${{1:item}} of {}) {{
  ${{0}}
}}"#,
                        name
                    )
                }),
            },
            SnippetCompletionItem {
                label: String::from("foreach"),
                detail: String::from("expr.forEach(item => )"),
                replace_string_generator: Box::new(|name| {
                    format!(
                        r#"{}.forEach(${{1:item}} => {{
    ${{0}}
}})"#,
                        name
                    )
                }),
            },
        ];
        snippet_list
            .into_iter()
            .map(|snippet| {
                let mut item = CompletionItem::new_simple(snippet.label, snippet.detail);
                item.insert_text_format = Some(InsertTextFormat::Snippet);
                item.kind = Some(CompletionItemKind::Snippet);
                let replace_string = (snippet.replace_string_generator)(source_code.clone());
                item.documentation = Some(Documentation::String(replace_string.clone()));

                item.insert_text = Some(replace_string);
                item.additional_text_edits =
                    Some(vec![TextEdit::new(replace_range.clone(), "".into())]);
                item
            })
            .collect()
    }
}
#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::Incremental,
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
        let TextDocumentItem {
            uri,
            language_id,
            version,
            text,
        } = params.text_document;
        self.document_map.lock().unwrap().insert(
            uri.to_string(),
            FullTextDocument::new(uri, language_id, version, text),
        );
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        // debug!("{:?}", params.content_changes);
        if let Some(document) = self
            .document_map
            .lock()
            .unwrap()
            .get_mut(&params.text_document.uri.to_string())
        {
            let changes = params
                .content_changes
                .into_iter()
                .map(|change| {
                    let range = change.range.and_then(|range| {
                        Some(lsp_types::Range {
                            start: lsp_types::Position::new(
                                range.start.line,
                                range.start.character,
                            ),
                            end: lsp_types::Position::new(range.end.line, range.end.character),
                        })
                    });
                    lsp_types::TextDocumentContentChangeEvent {
                        range,
                        range_length: change.range_length,
                        text: change.text,
                    }
                })
                .collect();
            let version = if let Some(version) = params.text_document.version {
                version
            } else {
                document.version
            };
            document.update(changes, version);
            // debug!("{:?}", document.text);
            // if let Some(content) = params.content_changes.first_mut().take() {
            //     document.text = content.text.clone();
            // }
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
                        let before_dot = Position::new(dot.line, dot.character.wrapping_sub(2));

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
                                if !node.is_error()
                                    && parent.kind().contains("expression")
                                    && parent.end_byte() == end_index
                                {
                                    node = parent;
                                } else {
                                    break;
                                }
                            }
                            let replace_range = Range::new(
                                Position::new(
                                    node.start_position().row as u64,
                                    node.start_position().column as u64,
                                ),
                                Position::new(dot.line, dot.character),
                            );

                            let source_code = &document.text[node.byte_range()];

                            let mut template_item_list = self.get_template_completion_item_list(
                                source_code.to_string(),
                                &replace_range,
                            );
                            template_item_list.extend(self.get_snippet_completion_item_list(
                                source_code.to_string(),
                                &replace_range,
                            ));
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
