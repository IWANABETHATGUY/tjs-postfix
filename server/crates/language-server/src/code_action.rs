use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use log::debug;
use lsp_text_document::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse, Range,
    TextEdit, WorkspaceEdit,
};
use serde::{Deserialize, Serialize};
use tower_lsp::jsonrpc;
use tree_sitter::{Language, Node, Point, Query, QueryCursor};

use crate::{helper::generate_lsp_range, query_pattern::FUNCTION_LIKE_DECLARATION, Backend};
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
    function_name: String,
}
pub async fn get_function_call_action(
    back_end: &Backend,
    params: CodeActionParams,
) -> Option<CodeActionResponse> {
    let mut ret = CodeActionResponse::new();
    let document_map = back_end.document_map.lock().await;
    let document = document_map.get(&params.text_document.uri.to_string())?;

    let map = back_end.parse_tree_map.lock().await;
    let tree = map.get(&params.text_document.uri.to_string())?;
    let duration = Instant::now();
    let root = tree.root_node();
    let range = params.range;
    let start = range.start;
    let end = range.end;

    // debug!("range_string, {:?}", document.rope.get_slice(star..end_char));
    // let start_char =
    //     document.rope.try_line_to_char(start.line as usize).ok()? + start.character as usize;
    // let end_char = document.rope.try_line_to_byte(end.line as usize).ok()? + end.character as usize;
    // let start_byte = document.rope.try_char_to_byte(start_char).ok()?;
    // let end_byte = document.rope.try_char_to_byte(end_char).ok()?;
    // debug!("text: {}", &document.get_text()[start_byte..end_byte]);
    let start_node = root.named_descendant_for_point_range(
        Point::new(start.line as usize, start.character as usize),
        Point::new(start.line as usize, start.character as usize + 1),
    )?;
    let end_node = root.named_descendant_for_point_range(
        Point::new(end.line as usize, end.character as usize),
        Point::new(end.line as usize, end.character as usize + 1),
    )?;
    let sp = start_node.parent()?;
    let ep = end_node.parent()?;
    if sp.kind() != "member_expression" || ep.kind() != "member_expression" {
        return None;
    }

    let start_object_node = sp.child_by_field_name("object");
    let end_object_node = ep.child_by_field_name("object");
    if let (Some(start), Some(_)) = (start_object_node, end_object_node) {
        let replace_range = generate_lsp_range(
            ep.start_position().row as u32,
            ep.start_position().column as u32,
            ep.end_position().row as u32,
            ep.end_position().column as u32,
        );
        // debug!("sp_parent: {}, ep_parent: {}", sp.kind(), ep.kind());
        let document_source = document.rope.to_string();
        let object_source_code = &document_source[start.byte_range()];

        let function = &document_source[start_node.start_byte()..end_node.end_byte()];

        let replaced_code = format!("{}({})", function, object_source_code);

        let edit = TextEdit::new(replace_range, replaced_code.clone());
        let mut changes = HashMap::new();
        changes.insert(params.text_document.uri, vec![edit]);
        ret.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: format!("call this function -> {}", replaced_code),
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            diagnostics: None,
            edit: Some(WorkspaceEdit::new(changes)),
            command: None,
            is_preferred: Some(false),
            disabled: None,
            data: None,
        }));
    }

    debug!("code-action: {:?}", duration.elapsed());

    Some(ret)
}

pub async fn extract_component_action(
    back_end: &Backend,
    params: CodeActionParams,
    code_action: &mut Vec<CodeActionOrCommand>,
) -> jsonrpc::Result<()> {
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
                .filter_map(|node| {
                    let parent = node.parent();
                    match parent {
                        Some(p)
                            if p.kind() == "jsx_opening_element"
                                || p.kind() == "jsx_closing_element"
                                || p.kind() == "nested_identifier" =>
                        {
                            return None;
                        }
                        None => return None,
                        _ => {}
                    };
                    let sp = node.start_position();
                    let ep = node.end_position();
                    Some(IdentifierNode {
                        start: node.start_byte(),
                        end: node.end_byte(),
                        range: generate_lsp_range(
                            sp.row as u32,
                            sp.column as u32,
                            ep.row as u32,
                            ep.column as u32,
                        ),
                        name: node.utf8_text(source.as_bytes()).unwrap().to_string(),
                    })
                })
                .collect::<Vec<_>>();
            let jsx_ele_sp = jsx_element_node.start_position();
            let jsx_ele_ep = jsx_element_node.end_position();
            let function_name =
                get_function_name_from_program(parser.language().unwrap(), source.as_bytes(), root);
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
                        function_name,
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

fn get_function_name_from_program<'b>(lang: Language, source: &[u8], root: Node<'b>) -> String {
    let jsx_query = Query::new(lang, FUNCTION_LIKE_DECLARATION).unwrap();

    let mut cursor = QueryCursor::new();
    // pretty_print(&source, node, 0);
    let jsx_matches = cursor.matches(&jsx_query, root, source);
    let mut id_set = HashSet::new();
    for item in jsx_matches {
        for cap in item.captures {
            if let Ok(id) = cap.node.utf8_text(source) {
                id_set.insert(id.to_string());
            }
        }
    }
    let mut i = 0;
    loop {
        let name = format!("Component{}", i);
        if id_set.contains(&name) {
            i += 1;
        } else {
            return name;
        }
    }
}
