use log::debug;
use lsp_text_document::FullTextDocument;
use lsp_types::{Position, TextDocumentContentChangeEvent};
use tree_sitter::{InputEdit, Node};
pub fn get_tree_sitter_edit_from_change(
    change: &TextDocumentContentChangeEvent,
    document: &mut FullTextDocument,
) -> Option<InputEdit> {
    if change.range.is_none() || change.range_length.is_none() {
        return None;
    }
    let range = change.range.unwrap();
    let text = change.text.clone();
    let range_length = change.range_length.unwrap();
    let start_byte = document.offset_at(Position {
        line: range.start.line,
        character: range.start.character,
    });
    let old_end_byte = start_byte + range_length as usize;
    let new_end_byte = start_byte + text.len();
    let new_end_position = document.position_at(new_end_byte as u32);
    let old_end_position = document.position_at(old_end_byte as u32);
    let start_position = document.position_at(start_byte as u32);
    Some(InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position: tree_sitter::Point {
            row: start_position.line as usize,
            column: start_position.character as usize,
        },
        old_end_position: tree_sitter::Point {
            row: old_end_position.line as usize,
            column: old_end_position.character as usize,
        },
        new_end_position: tree_sitter::Point {
            row: new_end_position.line as usize,
            column: new_end_position.character as usize,
        },
    })
}

pub fn pretty_print(source_code: &str, root: Node, level: usize) {
    if !root.is_named() {
        return
        // println!("{:?}", &source_code[root.start_byte()..root.end_byte()]);
    }
    let kind = root.kind();
    let start = root.start_position();
    let end = root.end_position();
    debug!(
        "{}{} [{}, {}] - [{}, {}] ",
        " ".repeat(level * 2),
        kind,
        start.row,
        start.column,
        end.row,
        end.column
    );
    for i in 0..root.child_count() {
        let node = root.child(i).unwrap();
        pretty_print(source_code, node, level + 1);
    }
}
