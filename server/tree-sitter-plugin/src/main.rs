use std::collections::HashSet;
use std::{fmt::Debug, path::PathBuf, time::Instant};
use std::{
    fs::read_to_string,
    sync::{Arc, Mutex},
};
use tree_sitter::{
    Language, Node, Parser, 
};
use tree_sitter_plugin::{ tree_sitter_scss,tree_sitter_tsx, tree_sitter_typescript};

fn main() {
    // let language = unsafe { tree_sitter_scss() };
    // let mut parser = Parser::new();
    // parser.set_language(language).unwrap();
    // let source = include_str!("../assets/small.scss");
    // let start = Instant::now();
    // let tree = parser.parse(&source, None).unwrap();
    // let time = Instant::now();

    // let node = tree.root_node();
    // pretty_print(&source, node, 0);
    // println!("{:?}", time.elapsed());
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
