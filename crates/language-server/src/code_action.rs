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
use streaming_iterator::StreamingIterator;
use tower_lsp::jsonrpc;
use tree_sitter::{Language, Node, Point, Query, QueryCursor, TextProvider};

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

const IDENTIFIER_QUERY_PATTERN: &str = r#"(identifier) @a"#;
const JSX_EXPRESSION_QUERY_PATTERN: &str = r#"(jsx_expression) @a"#;

fn get_function_name_from_program<'b>(lang: Language, source: &[u8], root: Node<'b>) -> String {
    let jsx_query = Query::new(&lang, FUNCTION_LIKE_DECLARATION).unwrap();

    let mut cursor = QueryCursor::new();
    // pretty_print(&source, node, 0);
    let mut jsx_matches = cursor.matches(&jsx_query, root, source);
    let mut id_set = HashSet::new();
    while let Some(item) = jsx_matches.next() {
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
