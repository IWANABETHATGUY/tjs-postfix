use dashmap::DashMap;
use ignore::Walk;
use lspower::{LspService, Server};
use notify_debouncer_mini::{notify, new_debouncer, DebounceEventResult};
use std::time::{Instant, Duration};
use tree_sitter::Parser;

use crossbeam_channel::unbounded;
use notify::{event::ModifyKind, Config, RecommendedWatcher, RecursiveMode, Result, Watcher};

use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex},
};
use tjs_language_server::{insert_position_list, remove_position_list, Backend, Job};
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

    let (mut tx, rx) = unbounded::<Job>();
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
            tx.clone(),
        )
    });

    let server = Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service);

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
            log::debug!("found {:?} css/scss/less file", scss_class_map.len());
            let config = Config::default();

            // let mut watcher = RecommendedWatcher::new(move |e| match e {
            //     Ok(e) => {
            //         tx.send(Job::Event(e)).unwrap();
            //     }
            //     Err(err) => {}
            // })?;
            let mut debouncer = new_debouncer(Duration::from_secs(2), None, |res: DebounceEventResult| {
                match res {
                    Ok(events) => events.iter().for_each(|e| {
                        tx.send(Job::Event(e.clone())).unwrap();
                    }),
                    Err(errors) => errors.iter().for_each(|e|println!("Error {:?}",e)),
                }
            }).unwrap();
            debouncer.watcher().watch(&work_dir, RecursiveMode::Recursive).unwrap();

            // std::mem::drop(&mut tx);
            // Add a path to be watched. All files and directories at that path and
            // below will be monitored for changes.
            // watcher.watch(&work_dir, RecursiveMode::Recursive)?;
            // watcher.configure(Config::NoticeEvents(true))?;
            loop {
                match rx.recv() {
                    Ok(Job::Event(e)) => {
                        let path_list = e
                            .path
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
                                let now = Instant::now();
                                path_list.into_iter().for_each(|p| {
                                    insert_position_list(&p, &mut parser, scss_class_map.clone());
                                });
                                log::debug!("reanalyze crate scss file cost {:?}", now.elapsed());
                            }
                            notify::EventKind::Modify(ModifyKind::Data(a)) => {
                                let now = Instant::now();
                                path_list.into_iter().for_each(|p| {
                                    insert_position_list(&p, &mut parser, scss_class_map.clone());
                                });
                                log::debug!(
                                    "reanalyze modify scss file cost {:?}, kind: {:?}",
                                    now.elapsed(),
                                    a
                                );
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
                    Ok(Job::Shutdown) => {
                        break;
                    }
                    Err(_) => todo!(),
                }
            }
        }
        Ok(())
    });
    let (_, _) = tokio::join!(scss_work_thread, server,);
}
