use std::{collections::HashMap, time::Instant};

pub use backend::TreeWrapper;
use helper::get_tree_sitter_edit_from_change;
// use helper::get_tree_sitter_edit_from_change;
use log::debug;
use lsp_text_document::lsp_types;
use lsp_text_document::FullTextDocument;
use lspower::jsonrpc;
use lspower::jsonrpc::Result;
use lspower::lsp::*;
use lspower::LanguageServer;
use notification::{AstPreviewRequestParams, CustomNotification, CustomNotificationParams};
use serde_json::Value;

mod backend;
mod code_action;
mod document_symbol;
mod helper;
mod notification;
mod query_pattern;

pub use backend::Backend;
use document_symbol::get_component_symbol;

use crate::helper::generate_lsp_range;
use code_action::{extract_component_action, get_function_call_action};

#[lspower::async_trait]
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
                    all_commit_characters: None,
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![],
                    work_done_progress_options: Default::default(),
                }),

                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::REFACTOR_REWRITE,
                        ]),
                        work_done_progress_options: WorkDoneProgressOptions {
                            work_done_progress: Some(true),
                        },
                        resolve_provider: Some(true),
                    },
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
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

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn request_else(
        &self,
        method: &str,
        _params: Option<serde_json::Value>,
    ) -> jsonrpc::Result<Option<serde_json::Value>> {
        if method == "tjs-postfix/ast-preview" {
            if let Some(params) = _params {
                let param = serde_json::from_value::<AstPreviewRequestParams>(params).unwrap();
                let path_ast_tuple =
                    if let Some(tree) = self.parse_tree_map.lock().await.get(&param.path) {
                        Some((param.path, format!("{}", TreeWrapper(tree.clone(),))))
                    } else {
                        None
                    };
                if let Some((path, ast_string)) = path_ast_tuple {
                    self.client
                        .send_custom_notification::<CustomNotification>(
                            CustomNotificationParams::new(path, ast_string),
                        )
                        .await;
                }
            }
        }
        Ok(None)
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

    async fn document_symbol(
        &self,
        params: lsp_types::DocumentSymbolParams,
    ) -> lspower::jsonrpc::Result<Option<lsp_types::DocumentSymbolResponse>> {
        get_component_symbol(&self, params).await
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let mut code_action_result = CodeActionResponse::new();
        get_function_call_action(&self, params.clone(), &mut code_action_result).await?;
        extract_component_action(&self, params, &mut code_action_result).await?;
        Ok(Some(code_action_result))
    }

    async fn execute_command(&self, _params: ExecuteCommandParams) -> Result<Option<Value>> {
        self.client
            .log_message(MessageType::Info, "command executed!")
            .await;

        Ok(None)
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let TextDocumentItem {
            uri,
            language_id,
            version,
            text,
        } = params.text_document;
        let tree = self.parser.lock().await.parse(&text, None).unwrap();
        self.parse_tree_map
            .lock()
            .await
            .insert(uri.to_string(), tree);
        self.document_map.lock().await.insert(
            uri.to_string(),
            FullTextDocument::new(uri, language_id, version as i64, text),
        );
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(document) = self
            .document_map
            .lock()
            .await
            .get_mut(&params.text_document.uri.to_string())
        {
            let mut parser = self.parser.lock().await;
            let mut parse_tree_map = self.parse_tree_map.lock().await;
            let changes: Vec<lsp_types::TextDocumentContentChangeEvent> = params
                .content_changes
                .into_iter()
                .map(|change| {
                    let range = change.range.map(|range| {
                        generate_lsp_range(
                            range.start.line as u32,
                            range.start.character as u32,
                            range.end.line as u32,
                            range.end.character as u32,
                        )
                    });
                    lsp_types::TextDocumentContentChangeEvent {
                        range,
                        range_length: change.range_length.and_then(|v| Some(v as u32)),
                        text: change.text,
                    }
                })
                .collect();
            let version = params.text_document.version;

            let tree = parse_tree_map
                .get_mut(&params.text_document.uri.to_string())
                .unwrap();
            let start = Instant::now();
            for change in changes {
                tree.edit(
                    &get_tree_sitter_edit_from_change(&change, document, version as i64).unwrap(),
                );
            }
            debug!("incremental updating: {:?}", start.elapsed());
            let new_tree = parser.parse(document.rope.to_string(), Some(tree)).unwrap();
            parse_tree_map.insert(params.text_document.uri.to_string(), new_tree);
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let start = Instant::now();
        let path = params.text_document.uri.to_string();
        let path_ast_tuple = if let Some(tree) = self.parse_tree_map.lock().await.get(&path) {
            Some((path, format!("{}", TreeWrapper(tree.clone(),))))
        } else {
            None
        };
        if let Some((path, ast_string)) = path_ast_tuple {
            self.client
                .send_custom_notification::<CustomNotification>(CustomNotificationParams::new(
                    path, ast_string,
                ))
                .await;
        }

        debug!("{:?}", start.elapsed());
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut map = self.document_map.lock().await;
        map.remove(&params.text_document.uri.to_string());
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        if let Some(_) = params.context {
            if let Some(document) = self
                .document_map
                .lock()
                .await
                .get(&params.text_document_position.text_document.uri.to_string())
            {
                let pos = params.text_document_position.position.clone();
                let line = document.rope.line(pos.line as usize);

                let line_text_before_cursor = line.slice(..pos.character as usize).to_string();
                let before_string = line_text_before_cursor
                    .rfind(".")
                    .and_then(|n| Some(&line_text_before_cursor[n + 1..]));
                // debug!("before_string:{:?}", before_string);
                if before_string.is_none() {
                    return Ok(None);
                }
                let map = self.parse_tree_map.lock().await;
                let tree = map.get(&params.text_document_position.text_document.uri.to_string());
                match tree {
                    Some(tree) => {
                        let completion_keyword = before_string.unwrap();
                        let start = Instant::now();
                        let root = tree.root_node();
                        let dot = params.text_document_position.position;
                        let before_dot = Position::new(
                            dot.line,
                            dot.character
                                .wrapping_sub(completion_keyword.len() as u32 + 2),
                        );
                        // this is based bytes index
                        // let byte_index_start = document.rope.line_to_byte(before_dot.line as usize);
                        let char_index = document.rope.line_to_char(before_dot.line as usize)
                            + before_dot.character as usize;
                        let byte_index = document.rope.char_to_byte(char_index);
                        let node = root.named_descendant_for_byte_range(byte_index, byte_index);

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
                            let replace_range = generate_lsp_range(
                                node.start_position().row as u32,
                                node.start_position().column as u32,
                                dot.line,
                                dot.character,
                            );
                            let source_code = &document.rope.to_string()[node.byte_range()];

                            let mut template_item_list = self.get_template_completion_item_list(
                                source_code.to_string(),
                                &replace_range,
                            );
                            template_item_list.extend(self.get_snippet_completion_item_list(
                                source_code.to_string(),
                                &replace_range,
                            ));
                            template_item_list.push(CompletionItem::new_simple(
                                format!("{:?}", start.elapsed()),
                                format!("{:?}: {:?}", node, Range::default()),
                            ));
                            return Ok(Some(CompletionResponse::Array(template_item_list)));
                        }
                    }
                    _ => {}
                };
            }
        }
        Ok(None)
    }
}
