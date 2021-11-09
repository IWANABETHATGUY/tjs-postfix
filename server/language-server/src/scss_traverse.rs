use tree_sitter::{Node, Point};

pub fn traverse_scss_file(
    root: Node,
    trace_stack: &mut Vec<Vec<String>>,
    source_code: &[u8],
    position_list: &mut Vec<(String, Point)>,
) {
    let kind = root.kind();
    match kind {
        "stylesheet" | "block" => {
            for i in 0..root.named_child_count() {
                let node = root.named_child(i).unwrap();
                traverse_scss_file(node, trace_stack, source_code, position_list);
            }
        }
        "rule_set" => {
            let selectors = root.child(0);
            let mut new_top = vec![];
            if let Some(selectors) = selectors {
                for index in 0..selectors.named_child_count() {
                    let selector = selectors.named_child(index).unwrap();
                    match selector.kind() {
                        "class_selector" => {
                            traverse_class_selector(
                                selector,
                                source_code,
                                trace_stack,
                                position_list,
                                &mut new_top,
                            );
                        }
                        "descendant_selector" => {
                            let mut cur_new_top = vec![];
                            for index in 0..selector.named_child_count() {
                                let s = selector.named_child(index).unwrap();
                                if s.kind() == "class_selector" {
                                    traverse_class_selector(
                                        s,
                                        source_code,
                                        trace_stack,
                                        position_list,
                                        &mut cur_new_top,
                                    );
                                }
                            }
                            new_top.push(cur_new_top.join(" "));
                        }
                        _ => {
                            // unimplemented!() // TODO
                        }
                    }
                }
            } else {
                return;
            }
            trace_stack.push(new_top);
            let block = root.child(1);
            if let Some(block) = block {
                traverse_scss_file(block, trace_stack, source_code, position_list);
            }
            trace_stack.pop();
        }
        _ => {}
    }
}

fn traverse_class_selector(
    selector: Node,
    source_code: &[u8],
    trace_stack: &mut Vec<Vec<String>>,
    position_list: &mut Vec<(String, Point)>,
    new_top: &mut Vec<String>,
) {
    let selector_content = selector
        .utf8_text(source_code)
        .unwrap()
        .to_string();
    let has_nested = selector_content.starts_with("&");
    if has_nested {
        // let partial = &class_name_content[1..];
        if let Some(class_list) = trace_stack.last() {
            for top_class in class_list {
                let class_name = format!("{}{}", &top_class, &selector_content[1..]);
                let selector_list = class_name
                    .split(".")
                    .filter(|a| !a.is_empty())
                    .collect::<Vec<_>>();
                for sub_selector in selector_list {
                    position_list
                        .push((sub_selector.trim().to_string(), selector.start_position()));
                }
                new_top.push(class_name);
            }
        }
    } else {
        let class_name = format!("{}", selector_content);
        let selector_list = class_name
            .split(".")
            .filter(|a| !a.is_empty())
            .collect::<Vec<_>>();
        for sub_selector in selector_list {
            position_list.push((sub_selector.trim().to_string(), selector.start_position()));
        }
        new_top.push(class_name);

        // position_list
        //     .push((selector_content.clone(), selector.start_position()));
        // new_top.push(selector_content);
    };
}

#[cfg(test)]
mod test_scss {
    use tree_sitter::Parser;

    use super::*;

    #[test]
    fn test_nesting_scss() {
        let scss = r#"
.btn {
    width: 100px;
    &-first.second, &-tes.that.third{
        height: 10px;
        &-result {
            color: #ccc;
            
        }
    }
}
        "#;
        let mut expected = vec![
            "btn".to_string(),
            "btn-first".to_string(),
            "second".to_string(),
            "btn-tes".to_string(),
            "that".to_string(),
            "third".to_string(),
            "btn-first".to_string(),
            "second-result".to_string(),
            "btn-tes".to_string(),
            "that".to_string(),
            "third-result".to_string(),
        ];
        expected.sort();
        assert_class_list(scss, expected);
    }

    #[test]
    fn test_descendent_scss() {
        let scss = r#"
.btn {
    width: 100px;
    &-result &-second, &-another.that {
        &.function {
            width: 10px;
        }
    }
}
        "#;
        let mut expected = vec![
            "btn-result".to_string(),
            "btn-second".to_string(),
            "btn-another".to_string(),
            "that".to_string(),
            "btn".to_string(),
            "btn-result".to_string(),
            "btn-second".to_string(),
            "function".to_string(),
            "btn-another".to_string(),
            "that".to_string(),
            "function".to_string(),
        ];
        expected.sort();
        assert_class_list(scss, expected);
    }

    fn assert_class_list(scss: &str, expected: Vec<String>) {
        let mut parser = Parser::new();
        let language = tree_sitter_scss::language();
        parser.set_language(language).unwrap();
        let tree = parser.parse(&scss, None).unwrap();
        let mut position_list = vec![];
        let root_node = tree.root_node();
        traverse_scss_file(root_node, &mut vec![], scss.as_bytes(), &mut position_list);
        let mut class_list = position_list
            .into_iter()
            .map(|item| item.0)
            .collect::<Vec<_>>();
        class_list.sort();
        assert_eq!(class_list, expected);
    }
}
