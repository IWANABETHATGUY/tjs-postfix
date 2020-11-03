  
use std::{fmt::Debug, path::PathBuf};
use tree_sitter::{Language, Node, Parser, TreeCursor};
use treesitter_ts::tree_sitter_typescript;

fn main() {
    let language = unsafe { tree_sitter_typescript() };
    let mut parser = Parser::new();
    parser.set_language(language).unwrap();
    let source_code = r#"
<template>
  <p>
    Hello, <a :[key]="url">{{ name 我的}}</a>!
  </p>
//  j 
</template>
"#;

    let tree = parser.parse(source_code, None).unwrap();
    let node = tree.root_node();
    pretty_print(&source_code, node, 0);
}

fn pretty_print(source_code: &str, root: Node, level: usize) {
    if !root.is_named() {
        println!("{:?}", &source_code[root.start_byte()..root.end_byte()]);
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