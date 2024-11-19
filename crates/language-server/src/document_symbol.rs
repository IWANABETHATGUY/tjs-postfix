use lsp_text_document::lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, Location, Position, Range, SymbolInformation,
    SymbolKind,
};

use tower_lsp::jsonrpc;
use tree_sitter::{Query, QueryCursor};

use crate::{query_pattern::DOCUMENT_SYMBOL_QUERY_PATTERN, Backend};
