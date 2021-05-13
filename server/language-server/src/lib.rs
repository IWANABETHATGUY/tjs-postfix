use std::{collections::HashMap, time::Instant};

pub use backend::TreeWrapper;
use helper::{get_tree_sitter_edit_from_change, pretty_print};
// use helper::get_tree_sitter_edit_from_change;
use log::{debug, error};
use lsp_text_document::FullTextDocument;
use lspower::jsonrpc::Result;
use lspower::lsp::*;
use lspower::LanguageServer;
// use notification::{CustomNotification, CustomNotificationParams};
use serde_json::Value;

mod backend;
mod helper;
mod notification;
pub use backend::Backend;

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
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
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
    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let mut code_action = CodeActionResponse::new();
        if let Some(document) = self
            .document_map
            .lock()
            .unwrap()
            .get(&params.text_document.uri.to_string())
        {
            let map = self.parse_tree_map.lock().unwrap();
            if let Some(tree) = map.get(&params.text_document.uri.to_string()) {
                {
                    let duration = Instant::now();
                    let root = tree.root_node();
                    let range = params.range;
                    let start = range.start;
                    let end = range.end;

                    let start_char =
                        document.rope.line_to_char(start.line as usize) + start.character as usize;
                    let end_char =
                        document.rope.line_to_char(end.line as usize) + end.character as usize;
                    let start_byte = document.rope.char_to_byte(start_char);
                    let end_byte = document.rope.char_to_byte(end_char);

                    let start_node = root.named_descendant_for_byte_range(start_byte, start_byte);
                    let end_node = root.named_descendant_for_byte_range(end_byte, end_byte);
                    if start_node.is_none() || end_node.is_none() {
                        return Ok(None);
                    }
                    let start_node = start_node.unwrap();
                    let end_node = end_node.unwrap();
                    if start_node.kind() != "property_identifier"
                        || end_node.kind() != "property_identifier"
                    {
                        return Ok(None);
                    }
                    match (start_node.parent(), end_node.parent()) {
                        (Some(sp), Some(ep))
                            if sp.kind() == "member_expression"
                                && ep.kind() == "member_expression" =>
                        {
                            let start_object_node = sp.child_by_field_name("object");
                            let end_object_node = ep.child_by_field_name("object");
                            if let (Some(start), Some(end)) = (start_object_node, end_object_node) {
                                let replace_range = Range::new(
                                    Position::new(
                                        ep.start_position().row as u32,
                                        ep.start_position().column as u32,
                                    ),
                                    Position::new(
                                        ep.end_position().row as u32,
                                        ep.end_position().column as u32,
                                    ),
                                );
                                let object_source_code =
                                    &document.rope.to_string()[start.byte_range()];

                                let function = &document.rope.to_string()
                                    [start_node.start_byte()..end_node.end_byte()];

                                let replaced_code = format!("{}({})", function, object_source_code);

                                let edit = TextEdit::new(replace_range, replaced_code.clone());
                                let mut changes = HashMap::new();
                                changes.insert(params.text_document.uri, vec![edit]);
                                code_action.push(CodeActionOrCommand::CodeAction(CodeAction {
                                    title: format!("call this function -> {}", replaced_code),
                                    kind: Some(CodeActionKind::REFACTOR_REWRITE),
                                    diagnostics: None,
                                    edit: Some(WorkspaceEdit::new(changes)),
                                    command: None,
                                    is_preferred: Some(false),
                                    disabled: None,
                                    data: None,
                                }));
                            } else {
                                return Ok(None);
                            }
                        }
                        _ => {
                            return Ok(None);
                        }
                    }
                    debug!("code-action: {:?}", duration.elapsed());
                    return Ok(Some(code_action));
                }
            }
        }
        unimplemented!() // TODO
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        self.client
            .log_message(MessageType::Info, "command executed!")
            .await;

        // match self.client.apply_edit(WorkspaceEdit::default()).await {
        //     Ok(res) if res.applied => self.client.log_message(MessageType::Info, "applied").await,
        //     Ok(_) => self.client.log_message(MessageType::Info, "rejected").await,
        //     Err(err) => self.client.log_message(MessageType::Error, err).await,
        // }

        Ok(None)
    }

    // async fn ast_preview(&self, params: PathParams) -> Result<()> {
    //     let path = params.path;
    //     let path_ast_tuple = if let Some(tree) = self.parse_tree_map.lock().unwrap().get(&path) {
    //         Some((path, format!("{}", TreeWrapper(tree.clone(),))))
    //     } else {
    //         None
    //     };
    //     if let Some((path, ast_string)) = path_ast_tuple {
    //         self.client
    //             .send_custom_notification::<CustomNotification>(CustomNotificationParams::new(
    //                 path, ast_string,
    //             ))
    //             .await;
    //     }
    //     Ok(())
    // }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let TextDocumentItem {
            uri,
            language_id,
            version,
            text,
        } = params.text_document;
        let tree = self.parser.lock().unwrap().parse(&text, None).unwrap();
        self.parse_tree_map
            .lock()
            .unwrap()
            .insert(uri.to_string(), tree);
        self.document_map.lock().unwrap().insert(
            uri.to_string(),
            FullTextDocument::new(uri, language_id, version as i64, text),
        );
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(document) = self
            .document_map
            .lock()
            .unwrap()
            .get_mut(&params.text_document.uri.to_string())
        {
            let mut parser = self.parser.lock().unwrap();
            let mut parse_tree_map = match self.parse_tree_map.lock() {
                Ok(map) => map,
                Err(_) => {
                    error!("can't hold the parse tree map lock");
                    return;
                }
            };
            let changes: Vec<lsp_types::TextDocumentContentChangeEvent> = params
                .content_changes
                .into_iter()
                .map(|change| {
                    let range = change.range.and_then(|range| {
                        Some(lsp_types::Range {
                            start: lsp_types::Position::new(
                                range.start.line as u32,
                                range.start.character as u32,
                            ),
                            end: lsp_types::Position::new(
                                range.end.line as u32,
                                range.end.character as u32,
                            ),
                        })
                    });
                    lsp_types::TextDocumentContentChangeEvent {
                        range,
                        range_length: change.range_length.and_then(|v| Some(v as u32)),
                        text: change.text,
                    }
                })
                .collect();
            let version =params.text_document.version;

            let tree = parse_tree_map
                .get_mut(&params.text_document.uri.to_string())
                .unwrap();
            let start = Instant::now();
            for change in changes {
                tree.edit(&get_tree_sitter_edit_from_change(&change, document, version as i64).unwrap());
                // debug!("{}", document.get_text());
            }
            debug!("incremental updating: {:?}", start.elapsed());
            let new_tree = parser.parse(document.rope.to_string(), Some(tree)).unwrap();
            parse_tree_map.insert(params.text_document.uri.to_string(), new_tree);
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let start = Instant::now();
        let path = params.text_document.uri.to_string();
        let path_ast_tuple = if let Some(tree) = self.parse_tree_map.lock().unwrap().get(&path) {
            Some((path, format!("{}", TreeWrapper(tree.clone(),))))
        } else {
            None
        };
        // if let Some((path, ast_string)) = path_ast_tuple {
        //     self.client
        //         .send_custom_notification::<CustomNotification>(CustomNotificationParams::new(
        //             path, ast_string,
        //         ))
        //         .await;
        // }

        debug!("{:?}", start.elapsed());
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut map = self.document_map.lock().unwrap();
        map.remove(&params.text_document.uri.to_string());
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        if let Some(context) = params.context {
            if let Some(document) = self
                .document_map
                .lock()
                .unwrap()
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
                let map = self.parse_tree_map.lock().unwrap();
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
                            let replace_range = Range::new(
                                Position::new(
                                    node.start_position().row as u32,
                                    node.start_position().column as u32,
                                ),
                                Position::new(dot.line, dot.character),
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
