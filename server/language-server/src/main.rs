use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tjs_language_server::Backend;
use tower_lsp::{LspService, Server};
use treesitter_ts::tree_sitter_typescript;
#[tokio::main]
async fn main() {
    env_logger::init();
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, messages) = LspService::new(|client| {
        let language = unsafe { tree_sitter_typescript() };
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(language).unwrap();
        let parser = Arc::new(Mutex::new(parser));
        let document_map = Arc::new(Mutex::new(HashMap::new()));
        let postfix_template_list = Arc::new(Mutex::new(vec![]));
        Backend::new(client, document_map, parser, postfix_template_list)
    });
    Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service)
        .await;
}
