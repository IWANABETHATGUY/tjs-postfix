use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use lspower::{jsonrpc::Result, lsp::*, Client, LanguageServer, LspService, Server};
use tjs_language_server::Backend;

use treesitter_ts::tree_sitter_tsx;
#[tokio::main]
async fn main() {
    env_logger::init();
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, messages) = LspService::new(|client| {
        let language = unsafe { tree_sitter_tsx() };
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(language).unwrap();
        let parser = Arc::new(Mutex::new(parser));
        let document_map = Arc::new(Mutex::new(HashMap::new()));
        let parse_tree_map = Arc::new(Mutex::new(HashMap::new()));
        let postfix_template_list = Arc::new(Mutex::new(vec![]));
        Backend::new(
            client,
            document_map,
            parser,
            postfix_template_list,
            parse_tree_map,
        )
    });
    Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service)
        .await;
    // Server::new(stdin, stdout)
    //     .interleave(messages)
    //     .serve(service)
    //     .await;
}
