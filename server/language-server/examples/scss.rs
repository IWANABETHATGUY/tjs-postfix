use std::time::Instant;

use tree_sitter::{Language, Node, Parser};
// use cssparser::{Parser as CssParser, ParserInput, Token};
fn main() {
    // let source_code = include_str!("../assets/bootstrap.css");
    let start = Instant::now();
    let mut parser = Parser::new();
    let language = unsafe { tree_sitter_scss::language() };
    parser.set_language(language).unwrap();
    let tree = parser.parse("", None).unwrap();
    println!("{:?}", start.elapsed());
    let root_node = tree.root_node();
    println!("{:?}", root_node.has_error());

    // let parser = CssParser::new(&mut ParserInput::new(source_code));
}

fn pretty_print(source_code: &str, root: Node, level: usize) {
    if !root.is_named() {
        return;
        // println!("{:?}", &source_code[root.start_byte()..root.end_byte()]);
    }
    let kind = root.kind();
    let start = root.start_position();
    let end = root.end_position();
    println!(
        "{}{} [{}, {}] - [{}, {}]<{}:{}>",
        " ".repeat(level * 2),
        kind,
        start.row,
        start.column,
        end.row,
        end.column,
        root.start_byte(),
        root.end_byte()
    );
    for i in 0..root.child_count() {
        let node = root.child(i).unwrap();
        pretty_print(source_code, node, level + 1);
    }
}
