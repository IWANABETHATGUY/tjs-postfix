use std::{fmt::Debug, path::PathBuf, time::Instant};
use std::{
    fs::read_to_string,
    sync::{Arc, Mutex},
};
use tree_sitter::{Language, Node, Parser, TreeCursor};
use treesitter_ts::{tree_sitter_tsx, tree_sitter_typescript};

fn main() {
    let language = unsafe { tree_sitter_tsx() };
    let language_typescript = unsafe { tree_sitter_typescript() };
    let mut parser = Parser::new();
    parser.set_language(language).unwrap();
    let parser = Arc::new(Mutex::new(parser));
    let res = read_to_string("test2.ts").unwrap();
    let mut parser = parser.lock().unwrap();
    // let start = Instant::now();
    let tree = parser.parse(&res, None).unwrap();
    // println!("{:?}", start.elapsed());
    // parser.set_language(language_typescript).unwrap();
    // let start = Instant::now();
    // let tree = parser.parse(&res, None).unwrap();
    // println!("{:?}", start.elapsed());
    // for i in 0..10 {
    //     let start = Instant::now();
    //     println!("{:?}", start.elapsed());
    // }
    println!("{:?}", tree);
    let node = tree.root_node();
    pretty_print(&res, node, 0);
}

fn pretty_print(source_code: &str, root: Node, level: usize) {
    if !root.is_named() {
        return
        // println!("{:?}", &source_code[root.start_byte()..root.end_byte()]);
    }
    let kind = root.kind();
    let start = root.start_position();
    let end = root.end_position();
    println!(
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
