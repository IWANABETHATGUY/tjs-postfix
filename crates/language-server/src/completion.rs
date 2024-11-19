use inflector::Inflector;
use streaming_iterator::StreamingIterator;
use tokio::sync::MutexGuard;
use tower_lsp::lsp_types::*;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

use crate::query_pattern::REACT_NAME_SPACE_IMPORT;

pub fn get_react_completion(
    name: &str,
    source: &str,
    replace_range: &Range,
    tree: &Tree,
    parser: MutexGuard<Parser>,
) -> Vec<CompletionItem> {
    let mut result = vec![];
    // let mut identifier_list = vec![];
    let function_call = if let Some(node) = get_react_import_node(
        &parser.language().unwrap(),
        source.as_bytes(),
        tree.root_node(),
    ) {
        if let Ok(id) = node.utf8_text(source.as_bytes()) {
            format!("{}.useState", id)
        } else {
            "useState".to_string()
        }
    } else {
        "useState".to_string()
    };

    let mut item = CompletionItem::new_simple(
        "state".to_string(),
        format!("const [<expr>, <expr>] = {}()", function_call),
    );
    item.kind = Some(CompletionItemKind::SNIPPET);
    let replace_string = format!(
        "const [{}, set{}] = {}(${{0}})",
        name,
        name.to_pascal_case(),
        function_call
    );
    item.documentation = Some(Documentation::String(replace_string.clone()));
    item.insert_text = Some(replace_string);
    item.insert_text_format = Some(InsertTextFormat::SNIPPET);
    item.additional_text_edits = Some(vec![TextEdit::new(replace_range.clone(), "".into())]);
    result.push(item);
    result
}

fn get_react_import_node<'a>(
    lang: &Language,
    source: &[u8],
    root: Node<'a>, // identifier_node_list: &'b mut Vec<Node<'a>>,
) -> Option<Node<'a>> {
    let jsx_expression_query = Query::new(&lang, &REACT_NAME_SPACE_IMPORT).unwrap();

    let mut cursor = QueryCursor::new();

    let mut jsx_expression_matches = cursor.matches(&jsx_expression_query, root, source);
    while let Some(item) = jsx_expression_matches.next() {
        for cap in item.captures {
            return Some(cap.node);
        }
    }
    None
}
