use codespan_lsp::{byte_index_to_position, position_to_byte_index};
use codespan_reporting::files::{Error, Files, SimpleFiles};
use log::debug;
use lsp_text_document::FullTextDocument;
use lsp_types::{Position, TextDocumentContentChangeEvent};
use tree_sitter::{InputEdit, Node};

pub fn get_tree_sitter_edit_from_change(
    change: &TextDocumentContentChangeEvent,
    document: &mut FullTextDocument,
    version: i64
) -> Option<InputEdit> {
    if change.range.is_none() || change.range_length.is_none() {
        return None;
    }
    let mut files = SimpleFiles::new();
    let before_file_id = files.add("before", document.text.clone());
    document.update(vec![change.clone()], version);
    
    let after_file_id = files.add("after", document.text.clone());
    // this is utf8 based bytes index
    let range = change.range.unwrap();
    let start = range.start;
    let end = range.end;
    let start_byte = position_to_byte_index(
        &files,
        before_file_id,
        &lsp_types::Position::new(start.line as u32, start.character as u32),
    )
    .unwrap();
    let old_end_byte = position_to_byte_index(
        &files,
        before_file_id,
        &lsp_types::Position::new(end.line as u32, end.character as u32),
    )
    .unwrap();
    let new_end_byte = start_byte + change.text.len();
    Some(InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position: byte_index_to_point(&files, before_file_id, start_byte).unwrap(),
        old_end_position: byte_index_to_point(&files, before_file_id, old_end_byte).unwrap(),
        new_end_position: byte_index_to_point(&files, after_file_id, new_end_byte).unwrap(),
    })
}

pub fn byte_index_to_point<'a, F>(
    files: &'a F,
    file_id: F::FileId,
    byte_index: usize,
) -> Result<tree_sitter::Point, Error>
where
    F: Files<'a> + ?Sized,
{
    // let source = files.source(file_id)?;
    // let source = source.as_ref();

    let line_index = files.line_index(file_id, byte_index)?;
    let line_span = files.line_range(file_id, line_index).unwrap();

    // let line_str = source
    //     .get(line_span.clone())
    //     .ok_or_else(|| Error::IndexTooLarge {
    //         given: if line_span.start >= source.len() {
    //             line_span.start
    //         } else {
    //             line_span.end
    //         },
    //         max: source.len() - 1,
    //     })?;
    let column = byte_index - line_span.start;
    Ok(tree_sitter::Point::new(line_index, column))
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

