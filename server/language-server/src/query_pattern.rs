
pub const DOCUMENT_SYMBOL_QUERY_PATTERN: &str = r#"
(jsx_opening_element
    name: (_) @a
    (#match? @a "^[A-Z]")
)
(jsx_self_closing_element
    name: (_) @a
    (#match? @a "^[A-Z]")
)
                "#;