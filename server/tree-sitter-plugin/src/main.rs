use std::collections::HashSet;
use std::{fmt::Debug, path::PathBuf, time::Instant};
use std::{
    fs::read_to_string,
    sync::{Arc, Mutex},
};
use tree_sitter::{
    Language, Node, Parser, Query, QueryCursor, QueryMatch, TextProvider, TreeCursor,
};
use tree_sitter_plugin::{tree_sitter_tsx, tree_sitter_typescript};

fn main() {
    let external_array = vec!["window"];
    let language = unsafe { tree_sitter_tsx() };
    let mut parser = Parser::new();
    parser.set_language(language).unwrap();
    let parser = Arc::new(Mutex::new(parser));
    let source = read_to_string("test.tsx").unwrap();
    let mut parser = parser.lock().unwrap();
    // let start = Instant::now();
    let jsx_pattern = r#"
(program
    [
        (function_declaration
            name: (identifier) @c
        )
        
        (_
            (variable_declarator
                name: (identifier) @c
                value: [
                    (function)
                    (arrow_function)
                ]
            )
        )
    ]
)
    "#;
    let tree = parser.parse(&source, None).unwrap();
    let time = Instant::now();
    let jsx_query = Query::new(language, &jsx_pattern).unwrap();

    let mut cursor = QueryCursor::new();
    let node = tree.root_node();
    // pretty_print(&source, node, 0);
    let mut jsx_matches = cursor.matches(&jsx_query, node, source.as_bytes());
    
    for item in jsx_matches {
        for cap in item.captures {
            println!("{}", cap.node.utf8_text(source.as_bytes()).unwrap());
        }
    }
    // let jsx_element = first_jsx_element.captures.iter().next().unwrap().node;

    // let res = cursor.matches(&jsx_expression_query, jsx_element, source.as_bytes());
    // for item in res {
    //     for cap in item.captures {
    //         let mut cursor = QueryCursor::new();
    //         let identifier_matches = cursor.matches(&local_query, cap.node, "".as_bytes());
    //         for id_match in identifier_matches {
    //             for inner_cap in id_match.captures {
    //                 println!("{}, ", inner_cap.node.utf8_text(source.as_bytes()).unwrap());
    //             }
    //         }
    //     }
    // }

    println!("{:?}", time.elapsed());
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
