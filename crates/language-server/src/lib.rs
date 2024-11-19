use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::time::Instant;

pub use backend::TreeWrapper;
use dashmap::DashMap;
use helper::get_tree_sitter_edit_from_change;
// use helper::get_tree_sitter_edit_from_change;
use jsonrpc::Result;
use log::debug;
use lsp_text_document::FullTextDocument;
use memmap2::Mmap;
use notification::{AstPreviewRequestParams, CustomNotification, CustomNotificationParams};
use notify::Event;
use serde_json::Value;
use tower_lsp::{jsonrpc, lsp_types::*, LanguageServer};
mod backend;
mod code_action;
mod completion;
mod document_symbol;
mod helper;
mod notification;
mod query_pattern;
mod scss_traverse;
pub use backend::Backend;
use document_symbol::get_component_symbol;
use tree_sitter::{Parser, Point};

pub enum Job {
    Event(Event),
    Shutdown,
}

use crate::helper::generate_lsp_range;
use crate::scss_traverse::traverse_scss_file;
use code_action::{extract_component_action, get_function_call_action};
use completion::get_react_completion;
#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        // *self.workspace_folder.lock().await = params.workspace_folders.unwrap_or(vec![]);
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "'".to_string(),
                        "\"".to_string(),
                    ]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![],
                    work_done_progress_options: Default::default(),
                }),

                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),

                document_symbol_provider: Some(OneOf::Left(true)),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                definition_provider: Some(OneOf::Right(DefinitionOptions {
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: Some(true),
                    },
                })),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.reset_templates().await;
        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        self.job_sender.send(Job::Shutdown).unwrap();
        Ok(())
    }

    // async fn request_else(
    //     &self,
    //     method: &str,
    //     _params: Option<serde_json::Value>,
    // ) -> jsonrpc::Result<Option<serde_json::Value>> {
    //     if method == "tjs-postfix/ast-preview" {
    //         if let Some(params) = _params {
    //             let param = serde_json::from_value::<AstPreviewRequestParams>(params).unwrap();
    //             let path_ast_tuple =
    //                 if let Some(tree) = self.parse_tree_map.lock().await.get(&param.path) {
    //                     Some((param.path, format!("{}", TreeWrapper(tree.clone(),))))
    //                 } else {
    //                     None
    //                 };
    //             if let Some((path, ast_string)) = path_ast_tuple {
    //                 self.client
    //                     .send_custom_notification::<CustomNotification>(
    //                         CustomNotificationParams::new(path, ast_string),
    //                     )
    //                     .await;
    //             }
    //         }
    //     }
    //     Ok(None)
    // }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        self.client
            .log_message(MessageType::INFO, "workspace folders changed!")
            .await;
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.reset_templates().await;
        self.client
            .log_message(MessageType::INFO, "configuration changed!")
            .await;
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        self.client
            .log_message(MessageType::INFO, "watched files have changed!")
            .await;
    }
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>> {
        if let Some(document) = self.document_map.lock().await.get(
            &params
                .text_document_position_params
                .text_document
                .uri
                .to_string(),
        ) {
            let pos = params.text_document_position_params.position;
            // debug!("before_string:{:?}", before_string);
            let map = self.parse_tree_map.lock().await;
            let tree = map.get(
                &params
                    .text_document_position_params
                    .text_document
                    .uri
                    .to_string(),
            );
            if let Some(tree) = tree {
                let root = tree.root_node();
                // this is based bytes index
                let char_index =
                    document.rope.line_to_char(pos.line as usize) + pos.character as usize;
                let click_byte = document.rope.char_to_byte(char_index);
                let node = root.named_descendant_for_byte_range(click_byte, click_byte);
                if let Some(click_node) = node {
                    // log::debug!("jump definition {:?}", click_node.kind());
                    // get latest jsx_attribute
                    if !matches!(click_node.kind(), "string_fragment" | "template_string") {
                        return Ok(None);
                    }
                    let attribute = {
                        let mut cur = click_node;
                        loop {
                            if cur.kind() == "jsx_attribute" {
                                break;
                            } else if cur.parent().is_none()
                                || matches!(
                                    cur.parent().unwrap().kind(),
                                    "ERROR" | "jsx_element" | "jsx_self_closing_element"
                                )
                            {
                                return Ok(None);
                            } else {
                                cur = cur.parent().unwrap();
                            }
                        }
                        cur
                    };

                    // match attribute.child(0) {
                    //     Some(prop) if prop.kind() == "property_identifier" => {
                    //         let document_content = document.rope.to_string();
                    //         if !matches!(
                    //             &document_content[prop.byte_range()],
                    //             "className" | "class"
                    //         ) {
                    //             log::error!("is not className");
                    //             return Ok(None);
                    //         }
                    //         let click_range = if click_node.kind() == "string_fragment" {
                    //             click_node.byte_range()
                    //         } else {
                    //             let range = click_node.byte_range();
                    //             range.start + 1..range.end - 1
                    //         };
                    //         let click_range_start = click_range.start;
                    //         let slice = &document_content[click_range];
                    //
                    //         let word_range = get_word_range_of_string(slice);
                    //
                    //         let range_index = word_range
                    //             .iter()
                    //             .find(|r| r.contains(&(click_byte - click_range_start)));
                    //         if let Some(class_name) = range_index.map(|range| &slice[range.clone()])
                    //         {
                    //             let mut locations = vec![];
                    //             for entry in self.scss_class_map.iter() {
                    //                 let path = entry.key();
                    //                 let point_list = entry.value();
                    //                 for (name, position) in point_list {
                    //                     if name == class_name {
                    //                         locations.push(Location::new(
                    //                             Url::parse(&format!("file://{}", path)).unwrap(),
                    //                             Range::new(
                    //                                 Position::new(
                    //                                     position.row as u32,
                    //                                     position.column as u32,
                    //                                 ),
                    //                                 Position::new(
                    //                                     position.row as u32,
                    //                                     position.column as u32,
                    //                                 ),
                    //                             ),
                    //                         ));
                    //                     }
                    //                 }
                    //             }
                    //             return Ok(Some(GotoDefinitionResponse::Array(locations)));
                    //         } else {
                    //             return Ok(None);
                    //         }
                    //     }
                    //     _ => (),
                    // }

                    // self.client
                    //     .log_message(MessageType::INFO, node.kind())
                    //     .await;
                }
            };
        }

        Ok(None)
    }

    // async fn document_symbol(
    //     &self,
    //     params: DocumentSymbolParams,
    // ) -> Result<Option<DocumentSymbolResponse>> {
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> jsonrpc::Result<Option<DocumentSymbolResponse>> {
        get_component_symbol(&self, params).await
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let mut code_action_result = CodeActionResponse::new();
        code_action_result.extend(
            get_function_call_action(&self, params.clone())
                .await
                .unwrap_or_default(),
        );
        extract_component_action(&self, params, &mut code_action_result).await?;
        Ok(Some(code_action_result))
    }

    async fn execute_command(&self, _params: ExecuteCommandParams) -> Result<Option<Value>> {
        self.client
            .log_message(MessageType::INFO, "command executed!")
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
            let changes: Vec<TextDocumentContentChangeEvent> = params
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
                    TextDocumentContentChangeEvent {
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

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {
        // let start = Instant::now();
        // let path = params.text_document.uri.to_string();
        // let path_ast_tuple = if let Some(tree) = self.parse_tree_map.lock().await.get(&path) {
        //     Some((path, format!("{}", TreeWrapper(tree.clone(),))))
        // } else {
        //     None
        // };
        // if let Some((path, ast_string)) = path_ast_tuple {
        //     self.client
        //         .send_custom_notification::<CustomNotification>(CustomNotificationParams::new(
        //             path, ast_string,
        //         ))
        //         .await;
        // }

        // debug!("{:?}", start.elapsed());
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
                let parser = self.parser.lock().await;
                let map = self.parse_tree_map.lock().await;
                let tree = map.get(&params.text_document_position.text_document.uri.to_string());

                match tree {
                    Some(tree) if before_string.is_some() => {
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
                        let before_dot_node =
                            root.named_descendant_for_byte_range(byte_index, byte_index);

                        if let Some(mut node) = before_dot_node {
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
                            let source = document.rope.to_string();

                            let res = get_react_completion(
                                &source[node.byte_range()],
                                &source,
                                &replace_range,
                                tree,
                                parser,
                            );
                            let mut template_item_list = self.get_template_completion_item_list(
                                &source[node.byte_range()],
                                &replace_range,
                            );
                            template_item_list.extend(self.get_snippet_completion_item_list(
                                &source[node.byte_range()],
                                &replace_range,
                            ));
                            template_item_list.extend(res);
                            template_item_list.push(CompletionItem::new_simple(
                                format!("{:?}", start.elapsed()),
                                format!("{:?}: {:?}", node, Range::default()),
                            ));
                            return Ok(Some(CompletionResponse::Array(template_item_list)));
                        }
                    }
                    Some(tree) => {
                        let root = tree.root_node();
                        let node = root.named_descendant_for_point_range(
                            Point::new(pos.line as usize, pos.character as usize),
                            Point::new(pos.line as usize, pos.character as usize),
                        );
                        if let Some(node) = node {
                            if matches!(node.kind(), "string" | "template_string") {
                                let attribute = {
                                    let mut cur = node;
                                    loop {
                                        if cur.kind() == "jsx_attribute" {
                                            break;
                                        } else if cur.parent().is_none()
                                            || matches!(
                                                cur.parent().unwrap().kind(),
                                                "ERROR"
                                                    | "jsx_element"
                                                    | "jsx_self_closing_element"
                                            )
                                        {
                                            return Ok(None);
                                        } else {
                                            cur = cur.parent().unwrap();
                                        }
                                    }
                                    cur
                                };
                                // match attribute.child(0) {
                                //     Some(prop) if prop.kind() == "property_identifier" => {
                                //         if !matches!(
                                //             &document.rope.to_string()[prop.byte_range()],
                                //             "className" | "class"
                                //         ) {
                                //             // log::debug!("is not className when completion");
                                //             return Ok(None);
                                //         }
                                //     }
                                //     _ => (),
                                // };
                                // let mut class_set = HashSet::new();
                                // for entry in self.scss_class_map.iter() {
                                //     for item in entry.value() {
                                //         class_set.insert(item.0.to_string());
                                //     }
                                // }
                                // let result = class_set
                                //     .into_iter()
                                //     .map(|class| {
                                //         let mut item =
                                //             CompletionItem::new_simple(class.clone(), class);
                                //         item.kind = Some(CompletionItemKind::CLASS);
                                //         item
                                //     })
                                //     .collect::<Vec<_>>();
                                return Ok(None);
                                // return Ok(Some(CompletionResponse::Array(result)));
                            } else {
                                return Ok(None);
                            };
                        }
                    }
                    _ => {}
                };
            }
        }
        Ok(None)
    }
}

pub fn get_word_range_of_string(string: &str) -> Vec<std::ops::Range<usize>> {
    let mut index = -1;
    let mut iter = string.bytes().enumerate();
    let mut range = vec![];
    let mut in_word = false;
    while let Some((i, c)) = iter.next() {
        if c.is_ascii_whitespace() && index != -1 {
            in_word = false;
            range.push(index as usize..i);
            index = -1;
        } else if !c.is_ascii_whitespace() && index == -1 {
            in_word = true;
            index = i as i32;
        }
    }

    if in_word {
        range.push(index as usize..string.len())
    }
    range
}

pub fn insert_position_list(
    path: &str,
    parser: &mut Parser,
    scss_class_map: Arc<DashMap<String, Vec<(String, Point)>>>,
) {
    if path.ends_with(".scss") || path.ends_with(".css") || path.ends_with(".less") {
        let read_time = Instant::now();
        match mmap_file_read(path) {
            Ok(file) => {
                log::debug!("read_file {:?}", read_time.elapsed());
                let mut start = Instant::now();
                let tree = parser.parse(&file, None).unwrap();
                log::debug!("parse time {:?}", start.elapsed());
                start = Instant::now();
                let mut position_list = vec![];
                let root_node = tree.root_node();
                traverse_scss_file(root_node, &mut vec![], &file, &mut position_list);
                log::debug!("traverse scss file {:?}", start.elapsed());
                scss_class_map.insert(path.to_string(), position_list);
            }
            Err(_) => {}
        }
    }
}

fn mmap_file_read(path: &str) -> std::io::Result<Vec<u8>> {
    let file = File::open(path)?;

    let mmap = unsafe { Mmap::map(&file).expect("failed to map the file") };
    let mut reader = Box::new(&mmap[..]);
    let mut buf = Vec::with_capacity(mmap.len());
    let mut buf_reader = std::io::BufReader::new(reader.as_mut());
    buf_reader.read_to_end(&mut buf)?;
    Ok(buf)
    // reader.concat()
}
pub fn remove_position_list(
    path: &str,
    scss_class_map: Arc<DashMap<String, Vec<(String, Point)>>>,
) {
    if path.ends_with(".scss") || path.ends_with(".css") || path.ends_with(".less") {
        scss_class_map.remove(path);
    }
}
