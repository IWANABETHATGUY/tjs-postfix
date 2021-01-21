use codespan_lsp::{byte_index_to_position, position_to_byte_index};
use codespan_reporting::files::{Error, Files, SimpleFiles};
use log::debug;
use lsp_text_document::FullTextDocument;
use lsp_types::{Position, TextDocumentContentChangeEvent};
use tree_sitter::{InputEdit, Node, Point};

pub fn get_tree_sitter_edit_from_change(
    change: &TextDocumentContentChangeEvent,
    document: &mut FullTextDocument,
    version: i64,
) -> Option<InputEdit> {
    if change.range.is_none() || change.range_length.is_none() {
        return None;
    }

    // this is utf8 based bytes index
    let range = change.range.unwrap();
    let start = range.start;
    let end = range.end;
    let start_char = document.rope.line_to_char(start.line as usize) + start.character as usize;
    let old_end_char = document.rope.line_to_char(end.line as usize) + end.character as usize;

    let start_byte = document.rope.char_to_byte(start_char);
    let old_end_byte = document.rope.char_to_byte(old_end_char);
    // let start_byte = document.rope.line
    // let start_byte = position_to_byte_index(
    //     &files,
    //     file_id,
    //     &lsp_types::Position::new(start.line as u32, start.character as u32),
    // )
    // .unwrap();
    // let old_end_byte = position_to_byte_index(
    //     &files,
    //     file_id,
    //     &lsp_types::Position::new(end.line as u32, end.character as u32),
    // )
    // .unwrap();
    document.update(vec![change.clone()], version);
    let new_end_char = start_char + change.text.chars().count();
    let new_end_byte = document.rope.char_to_byte(new_end_char);

    let new_end_line = document.rope.char_to_line(new_end_char);
    let new_end_line_first_character = document.rope.line_to_char(new_end_line);
    let new_end_character = new_end_byte - new_end_line_first_character;
    Some(InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position: Point::new(start.line as usize, start.character as usize),
        old_end_position: Point::new(end.line as usize, end.character as usize),
        new_end_position: Point::new(new_end_line, new_end_character),
    })
}

pub fn pretty_print(source_code: &str, root: Node, level: usize) {
    if !root.is_named() {
        return;
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
