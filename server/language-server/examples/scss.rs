use std::time::Instant;

use tree_sitter::{Language, Node, Parser};
// use cssparser::{Parser as CssParser, ParserInput, Token};
fn main() {
    let source_code = include_str!("../assets/bootstrap.css");
    let start = Instant::now();
    let mut parser = Parser::new();
    let language = tree_sitter_scss::language();
    parser.set_language(language).unwrap();
    let tree = parser.parse(source_code, None).unwrap();
    println!("{:?}", start.elapsed());
    let mut position_list = vec![];
    let root_node = tree.root_node();
    println!("{:?}", root_node.has_error());
    let start = Instant::now();
    traverse(root_node, &mut vec![], source_code, &mut position_list);
    println!("{:?}", start.elapsed());
    // println!("{:?}", position_list);
    // let parser = CssParser::new(&mut ParserInput::new(source_code));
}

fn traverse(
    root: Node,
    trace_stack: &mut Vec<Vec<String>>,
    source_code: &str,
    position_list: &mut Vec<(String, usize)>,
) {
    let kind = root.kind();
    match kind {
        "stylesheet" | "block" => {
            for i in 0..root.named_child_count() {
                let node = root.named_child(i).unwrap();
                traverse(node, trace_stack, source_code, position_list);
            }
        }
        "rule_set" => {
            let selectors = root.child(0);
            let mut new_top = vec![];
            if let Some(selectors) = selectors {
                // println!("{:?}", selectors);
                for index in 0..selectors.named_child_count() {
                    let selector = selectors.named_child(index).unwrap();
                    match selector.kind() {
                        "class_selector" => {
                            let content = selector
                                .utf8_text(source_code.as_bytes())
                                .unwrap()
                                .to_string();
                            if content.starts_with("&") {
                                let partial = &content[1..];
                                if let Some(class_list) = trace_stack.last() {
                                    for top_class in class_list {
                                        let class_name = format!("{}{}", top_class, partial);
                                        position_list.push((class_name.clone(), selector.start_byte()));
                                        new_top.push(class_name);
                                    }
                                }
                            } else {
                                position_list.push((content.clone(), selector.start_byte()));
                                new_top.push(content);
                            };
                        }
                        _ => {
                            // unimplemented!() // TODO
                        }
                    }
                }
            } else {
                return;
            }
            // println!("{:?}", new_top);
            trace_stack.push(new_top);
            let block = root.child(1);
            if let Some(block) = block {
                traverse(block, trace_stack, source_code, position_list);
            }
            trace_stack.pop();
        }
        _ => {}
    }
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
