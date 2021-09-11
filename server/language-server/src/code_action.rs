use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use log::debug;
use lsp_text_document::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, Position, Range, TextEdit,
    WorkspaceEdit,
};
use lspower::jsonrpc::Result;
use serde::{Deserialize, Serialize};
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

use crate::{helper::generate_lsp_range, Backend};
#[derive(Serialize, Deserialize)]
pub struct IdentifierNode {
    start: usize,
    end: usize,
    range: Range,
    name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractComponentData {
    jsx_element_range: Range,
    identifier_node_list: Vec<IdentifierNode>,
}
pub async fn get_function_call_action(
    back_end: &Backend,
    params: CodeActionParams,
    code_action: &mut Vec<CodeActionOrCommand>,
) -> Result<()> {
    if let Some(document) = back_end
        .document_map
        .lock()
        .await
        .get(&params.text_document.uri.to_string())
    {
        let map = back_end.parse_tree_map.lock().await;
        if let Some(tree) = map.get(&params.text_document.uri.to_string()) {
            let duration = Instant::now();
            let root = tree.root_node();
            let range = params.range;
            let start = range.start;
            let end = range.end;

            let start_char =
                document.rope.line_to_char(start.line as usize) + start.character as usize;
            let end_char = document.rope.line_to_char(end.line as usize) + end.character as usize;
            let start_byte = document.rope.char_to_byte(start_char);
            let end_byte = document.rope.char_to_byte(end_char);

            let start_node = root.named_descendant_for_byte_range(start_byte, start_byte);
            let end_node = root.named_descendant_for_byte_range(end_byte, end_byte);
            if start_node.is_none() || end_node.is_none() {
                return Ok(());
            }
            let start_node = start_node.unwrap();
            let end_node = end_node.unwrap();
            if start_node.kind() != "property_identifier"
                || end_node.kind() != "property_identifier"
            {
                return Ok(());
            }
            match (start_node.parent(), end_node.parent()) {
                (Some(sp), Some(ep))
                    if sp.kind() == "member_expression" && ep.kind() == "member_expression" =>
                {
                    let start_object_node = sp.child_by_field_name("object");
                    let end_object_node = ep.child_by_field_name("object");
                    if let (Some(start), Some(_)) = (start_object_node, end_object_node) {
                        let replace_range = generate_lsp_range(
                            ep.start_position().row as u32,
                            ep.start_position().column as u32,
                            ep.end_position().row as u32,
                            ep.end_position().column as u32,
                        );
                        let document_source = document.rope.to_string();
                        let object_source_code = &document_source[start.byte_range()];

                        let function =
                            &document_source[start_node.start_byte()..end_node.end_byte()];

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
                        return Ok(());
                    }
                }
                _ => {
                    return Ok(());
                }
            }
            debug!("code-action: {:?}", duration.elapsed());
            return Ok(());
        }
    }
    Ok(())
}

pub async fn extract_component_action(
    back_end: &Backend,
    params: CodeActionParams,
    code_action: &mut Vec<CodeActionOrCommand>,
) -> Result<()> {
    if let Some(document) = back_end
        .document_map
        .lock()
        .await
        .get(&params.text_document.uri.to_string())
    {
        let map = back_end.parse_tree_map.lock().await;
        let parser = back_end.parser.lock().await;
        if let Some(tree) = map.get(&params.text_document.uri.to_string()) {
            // let duration = Instant::now();
            let root = tree.root_node();
            let range = params.range;
            let start = range.start;
            let end = range.end;

            let start_char =
                document.rope.line_to_char(start.line as usize) + start.character as usize;
            let end_char = document.rope.line_to_char(end.line as usize) + end.character as usize;
            let start_byte = document.rope.char_to_byte(start_char);
            let end_byte = document.rope.char_to_byte(end_char);
            let source = document.rope.to_string();
            let node = root.named_descendant_for_byte_range(start_byte, end_byte);
            // let end_node = root.named_descendant_for_byte_range(end_byte, end_byte);
            if node.is_none() {
                return Ok(());
            }
            let node = node.unwrap();
            // let mut identifier_list = vec![];
            let node_kind = node.kind();
            let identifier_list = match node_kind {
                "jsx_element" | "jsx_self_closing_element" => get_identifier_from_jsx_element(
                    parser.language().unwrap(),
                    source.as_bytes(),
                    node,
                ),
                _ => {
                    return Ok(());
                }
            };
            let jsx_element_node = node;
            let identifier_node_list = identifier_list
                .into_iter()
                .map(|node| {
                    let sp = node.start_position();
                    let ep = node.end_position();
                    IdentifierNode {
                        start: node.start_byte(),
                        end: node.end_byte(),
                        range: generate_lsp_range(
                            sp.row as u32,
                            sp.column as u32,
                            ep.row as u32,
                            ep.column as u32,
                        ),
                        name: node.utf8_text(source.as_bytes()).unwrap().to_string(),
                    }
                })
                .collect::<Vec<_>>();
            let jsx_ele_sp = jsx_element_node.start_position();
            let jsx_ele_ep = jsx_element_node.end_position();
            code_action.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: "extract react component".to_string(),
                kind: Some(CodeActionKind::REFACTOR_REWRITE),
                diagnostics: None,
                edit: None,
                command: None,
                is_preferred: Some(false),
                disabled: None,
                data: Some(
                    serde_json::to_value(ExtractComponentData {
                        identifier_node_list,
                        jsx_element_range: generate_lsp_range(
                            jsx_ele_sp.row as u32,
                            jsx_ele_sp.column as u32,
                            jsx_ele_ep.row as u32,
                            jsx_ele_ep.column as u32,
                        ),
                    })
                    .unwrap(),
                ),
            }));
            // debug!("code-action: {:?}", duration.elapsed());
            return Ok(());
        }
    }
    Ok(())
}

const IDENTIFIER_QUERY_PATTERN: &str = r#"(identifier) @a"#;
const JSX_EXPRESSION_QUERY_PATTERN: &str = r#"(jsx_expression) @a"#;

fn get_identifier_from_jsx_element<'a, 'b: 'a>(
    lang: Language,
    source: &[u8],
    jsx_element: Node<'b>,
    // identifier_node_list: &'b mut Vec<Node<'a>>,
) -> Vec<Node<'a>> {
    let mut vec = vec![];
    // let start = Instant::now();
    let mut id_set = HashSet::new();
    let local_query = Query::new(lang, &IDENTIFIER_QUERY_PATTERN).unwrap();
    let jsx_expression_query = Query::new(lang, &JSX_EXPRESSION_QUERY_PATTERN).unwrap();

    let mut cursor = QueryCursor::new();
    // pretty_print(&source, node, 0);

    let jsx_expression_matches = cursor.matches(&jsx_expression_query, jsx_element, source);
    for item in jsx_expression_matches {
        for cap in item.captures {
            let mut cursor = QueryCursor::new();
            let identifier_matches = cursor.matches(&local_query, cap.node, "".as_bytes());
            for id_match in identifier_matches {
                for inner_cap in id_match.captures {
                    let name = inner_cap.node.utf8_text(source).unwrap().to_string();
                    if !id_set.contains(&name) {
                        vec.push(inner_cap.node);
                        id_set.insert(name);
                    }
                }
            }
        }
    }
    return vec;
}
