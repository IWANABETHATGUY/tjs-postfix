use dashmap::DashMap;
use ignore::Walk;
use lspower::{LspService, Server};
use std::{ffi::OsStr, fs::read_to_string, path::Path};
use tree_sitter::{Node, Parser, Point};

use crossbeam_channel::unbounded;
use notify::{Config, RecommendedWatcher, RecursiveMode, Result, Watcher};

use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};
use tjs_language_server::Backend;
use tokio::sync::Mutex;

use tree_sitter_typescript::language_tsx;

#[tokio::main]
async fn main() {
    env_logger::init();
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let tsx_lang = language_tsx();
    let mut tsx_parser = tree_sitter::Parser::new();
    tsx_parser.set_language(tsx_lang).unwrap();
    let scss_class_map = Arc::new(DashMap::new());
    let (service, messages) = LspService::new(|client| {
        let document_map = Mutex::new(HashMap::new());
        let parse_tree_map = Mutex::new(HashMap::new());
        let postfix_template_list = Arc::new(StdMutex::new(vec![]));
        Backend::new(
            client,
            document_map,
            Mutex::new(tsx_parser),
            postfix_template_list,
            parse_tree_map,
            scss_class_map.clone(),
        )
    });

    let scss_work_thread = tokio::task::spawn_blocking(move || -> Result<()> {
        // TODO: should use workdir of vscode
        if let Ok(work_dir) = std::env::current_dir() {
            let mut parser = Parser::new();
            let language = tree_sitter_scss::language();
            parser.set_language(language).unwrap();
            for result in Walk::new(work_dir.clone()) {
                match result {
                    Ok(entry) => {
                        let path = entry.path().display().to_string();
                        insert_position_list(&path, &mut parser, scss_class_map.clone());
                    }
                    Err(err) => log::debug!("ERROR: {}", err),
                }
            }
            log::debug!("found {:?} css/scss file", scss_class_map.len());
            let (tx, rx) = unbounded();
            let mut watcher = RecommendedWatcher::new(move |e| match e {
                Ok(e) => {
                    tx.send(e).unwrap();
                }
                Err(err) => {}
            })?;
            // Add a path to be watched. All files and directories at that path and
            // below will be monitored for changes.
            watcher.watch(&work_dir, RecursiveMode::Recursive)?;
            watcher.configure(Config::NoticeEvents(true))?;
            loop {
                match rx.recv() {
                    Ok(e) => {
                        let path_list = e
                            .paths
                            .into_iter()
                            .filter_map(|item| {
                                item.canonicalize()
                                    .ok()
                                    .and_then(|item| item.into_os_string().into_string().ok())
                                    .map(|item| item.to_string())
                            })
                            .collect::<Vec<_>>();
                        match e.kind {
                            notify::EventKind::Create(kind) => {
                                path_list.into_iter().for_each(|p| {
                                    insert_position_list(&p, &mut parser, scss_class_map.clone());
                                });
                            }
                            notify::EventKind::Modify(kind) => {
                                path_list.into_iter().for_each(|p| {
                                    insert_position_list(&p, &mut parser, scss_class_map.clone());
                                });
                            }
                            notify::EventKind::Remove(kind) => {
                                path_list.into_iter().for_each(|p| {
                                    remove_position_list(&p, scss_class_map.clone());
                                });
                            }
                            _ => {}
                        }
                        // println!("{:?}", e);
                    }
                    Err(_) => todo!(),
                }
            }
        }
        Ok(())
    });
    let server = Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service);

    let (a, b) = tokio::join!(scss_work_thread, server,);
}

fn traverse(
    root: Node,
    trace_stack: &mut Vec<Vec<String>>,
    source_code: &str,
    position_list: &mut Vec<(String, Point)>,
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
                for index in 0..selectors.named_child_count() {
                    let selector = selectors.named_child(index).unwrap();
                    match selector.kind() {
                        "class_selector" => {
                            // get class_name of selector
                            let (class_name, has_nested) = {
                                let mut class_name = None;
                                let mut has_nested = false;
                                for ci in 0..selector.named_child_count() {
                                    let c = selector.named_child(ci).unwrap();
                                    if c.kind() == "class_name" {
                                        class_name = Some(c);
                                    }
                                    if c.kind() == "nesting_selector" {
                                        has_nested = true;
                                    }
                                }
                                (class_name, has_nested)
                            };
                            if class_name.is_none() {
                                continue;
                            }
                            let class_name_content = class_name
                                .unwrap()
                                .utf8_text(source_code.as_bytes())
                                .unwrap()
                                .to_string();
                            if has_nested {
                                // let partial = &class_name_content[1..];
                                if let Some(class_list) = trace_stack.last() {
                                    for top_class in class_list {
                                        let class_name =
                                            format!("{}{}", top_class, class_name_content);
                                        position_list
                                            .push((class_name.clone(), selector.start_position()));
                                        new_top.push(class_name);
                                    }
                                }
                            } else {
                                position_list
                                    .push((class_name_content.clone(), selector.start_position()));
                                new_top.push(class_name_content);
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
fn insert_position_list(
    path: &str,
    parser: &mut Parser,
    scss_class_map: Arc<DashMap<String, Vec<(String, Point)>>>,
) {
    if path.ends_with(".scss") || path.ends_with(".css") {
        match read_to_string(&path) {
            Ok(file) => {
                let tree = parser.parse(&file, None).unwrap();
                let mut position_list = vec![];
                let root_node = tree.root_node();
                traverse(root_node, &mut vec![], &file, &mut position_list);
                scss_class_map.insert(path.to_string(), position_list);
            }
            Err(_) => {}
        }
    }
}

fn remove_position_list(path: &str, scss_class_map: Arc<DashMap<String, Vec<(String, Point)>>>) {
    if path.ends_with(".scss") || path.ends_with(".css") {
        scss_class_map.remove(path);
    }
}
