use tower_lsp::{LspService, Server};

use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex},
};
use tjs_language_server::Backend;
use tokio::sync::Mutex;

use tree_sitter_typescript::LANGUAGE_TSX;

#[tokio::main]
async fn main() {
    env_logger::init();
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let mut tsx_parser = tree_sitter::Parser::new();
    tsx_parser.set_language(&LANGUAGE_TSX.into()).unwrap();

    let (service, socket) = LspService::new(|client| {
        let document_map = Mutex::new(HashMap::new());
        let parse_tree_map = Mutex::new(HashMap::new());
        let postfix_template_list = Arc::new(StdMutex::new(vec![]));
        Backend::new(
            client,
            document_map,
            Mutex::new(tsx_parser),
            postfix_template_list,
            parse_tree_map,
        )
    });

    let server = Server::new(stdin, stdout, socket).serve(service);

    server.await;
}
