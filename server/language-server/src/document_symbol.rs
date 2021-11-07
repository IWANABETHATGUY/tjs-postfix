use lsp_text_document::lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, Location, Position, Range, SymbolInformation,
    SymbolKind,
};

use lspower::jsonrpc::Result;
use tree_sitter::{Query, QueryCursor};

use crate::{query_pattern::DOCUMENT_SYMBOL_QUERY_PATTERN, Backend};

pub async fn get_component_symbol(
    backend: &Backend,
    params: DocumentSymbolParams,
) -> Result<Option<DocumentSymbolResponse>> {
    if let Some(document) = backend
        .document_map
        .lock()
        .await
        .get_mut(&params.text_document.uri.to_string())
    {
        let res = if let Some(tree) = backend
            .parse_tree_map
            .lock()
            .await
            .get(&params.text_document.uri.to_string())
        {
            let parser = backend.parser.lock().await;

            let query =
                Query::new(parser.language().unwrap(), &DOCUMENT_SYMBOL_QUERY_PATTERN).unwrap();
            let mut cursor = QueryCursor::new();
            let node = tree.root_node();
            let mut symbol_infos = vec![];
            let source = document.rope.to_string();
            let source_bytes = source.as_bytes();
            let res = cursor.captures(&query, node, source_bytes);
            for item in res {
                for cap in item.0.captures {
                    let current_node = cap.node;
                    if let Ok(name) = current_node.utf8_text(source_bytes) {
                        symbol_infos.push(SymbolInformation {
                            name: name.to_string(),
                            kind: SymbolKind::Operator,
                            tags: None,
                            location: Location {
                                uri: params.text_document.uri.clone(),
                                range: Range::new(
                                    Position::new(
                                        current_node.start_position().row as u32,
                                        current_node.start_position().column as u32,
                                    ),
                                    Position::new(
                                        current_node.end_position().row as u32,
                                        current_node.end_position().column as u32,
                                    ),
                                ),
                            },
                            container_name: None,
                            deprecated: None,
                        });
                    }
                }
            }
            Ok(Some(DocumentSymbolResponse::Flat(symbol_infos)))
        } else {
            Ok(None)
        };
        return res;
    } else {
        Ok(None)
    }
}
