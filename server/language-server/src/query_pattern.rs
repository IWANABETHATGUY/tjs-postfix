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

pub const LOCAL_VARIABLE_QUERY: &str = r#"
(lexical_declaration
  (variable_declarator
  	name: (identifier) @d
  )
)
(function_declaration
  name: (identifier) @a
) 
            "#;

pub const REACT_NAME_SPACE_IMPORT: &str = r#"
(import_statement
  (import_clause
    (namespace_import (identifier) @namespace)
  )
  source: (string
    (string_fragment) @source
  )
  
  (#match? @source "react")
)
"#;
