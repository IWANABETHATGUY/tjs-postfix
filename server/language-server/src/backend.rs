use lsp_text_document::FullTextDocument;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::{Arc, Mutex}};
use tree_sitter::{Parser, Tree};
use tower_lsp::{Client, lsp_types::*};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PostfixTemplate {
    snippet_key: String,
    function_name: String,
}

pub struct SnippetCompletionItem {
    label: String,
    detail: String,
    replace_string_generator: Box<dyn Fn(String) -> String>,
}
pub struct Backend {
    pub(crate) client: Client,
    pub(crate) document_map: Arc<Mutex<HashMap<String, FullTextDocument>>>,
    pub(crate) parser: Arc<Mutex<Parser>>,
    pub(crate) parse_tree_map: Arc<Mutex<HashMap<String, Tree>>>,
    postfix_template_list: Arc<Mutex<Vec<PostfixTemplate>>>,
}
impl Backend {
    pub fn new(
        client: Client,
        document_map: Arc<Mutex<HashMap<String, FullTextDocument>>>,
        parser: Arc<Mutex<Parser>>,
        postfix_template_list: Arc<Mutex<Vec<PostfixTemplate>>>,
        parse_tree_map: Arc<Mutex<HashMap<String, Tree>>>,
    ) -> Self {
        Self {
            client,
            document_map,
            parser,
            postfix_template_list,
            parse_tree_map,
        }
    }

    pub(crate) async fn reset_templates(&self) {
        let configuration = self
            .client
            .configuration(vec![ConfigurationItem {
                scope_uri: None,
                section: Some("tjs-postfix.templateMapList".into()),
            }])
            .await;
        if let Ok(mut configuration) = configuration {
            if let Ok(configuration) = serde_json::from_value::<Vec<PostfixTemplate>>(
                configuration.first_mut().unwrap().take(),
            ) {
                match self.postfix_template_list.lock() {
                    Ok(mut list) => {
                        list.clear();
                        list.extend(configuration);
                    }
                    Err(_) => {}
                }
            }
        }
    }

    pub(crate) fn get_template_completion_item_list(
        &self,
        source_code: String,
        replace_range: &Range,
    ) -> Vec<CompletionItem> {
        if let Ok(template_list) = self.postfix_template_list.lock() {
            template_list
                .iter()
                .map(|template_item| {
                    let mut item = CompletionItem::new_simple(
                        template_item.snippet_key.clone(),
                        template_item.function_name.clone(),
                    );
                    item.kind = Some(CompletionItemKind::Snippet);
                    let replace_string =
                        format!("{}({})", &template_item.function_name, source_code);
                    item.documentation = Some(Documentation::String(replace_string.clone()));
                    item.insert_text = Some(replace_string);
                    item.additional_text_edits =
                        Some(vec![TextEdit::new(replace_range.clone(), "".into())]);
                    item
                })
                .collect()
        } else {
            vec![]
        }
    }

    pub(crate) fn get_snippet_completion_item_list(
        &self,
        source_code: String,
        replace_range: &Range,
    ) -> Vec<CompletionItem> {
        let snippet_list = vec![
            SnippetCompletionItem {
                label: String::from("not"),
                detail: String::from("revert a variable or expression"),
                replace_string_generator: Box::new(|name| format!("!{}", name)),
            },
            SnippetCompletionItem {
                label: String::from("if"),
                detail: String::from("if (expr)"),
                replace_string_generator: Box::new(|name| {
                    format!(
                        r#"if ({}) {{
    ${{0}}
}}"#,
                        name
                    )
                }),
            },
            SnippetCompletionItem {
                label: String::from("var"),
                detail: String::from("var name = expr"),
                replace_string_generator: Box::new(|name| format!("var ${{0}} = {}", name)),
            },
            SnippetCompletionItem {
                label: String::from("let"),
                detail: String::from("let name = expr"),
                replace_string_generator: Box::new(|name| format!("let ${{0}} = {}", name)),
            },
            SnippetCompletionItem {
                label: String::from("const"),
                detail: String::from("const name = expr"),
                replace_string_generator: Box::new(|name| format!("const ${{0}} = {}", name)),
            },
            SnippetCompletionItem {
                label: String::from("cast"),
                detail: String::from("(<name>expr)"),
                replace_string_generator: Box::new(|name| format!("(<${{0}}>{})", name)),
            },
            SnippetCompletionItem {
                label: String::from("as"),
                detail: String::from("(expr as name)"),
                replace_string_generator: Box::new(|name| format!("({} as ${{0}})", name)),
            },
            SnippetCompletionItem {
                label: String::from("new"),
                detail: String::from("new expr()"),
                replace_string_generator: Box::new(|name| format!("new {}()", name)),
            },
            SnippetCompletionItem {
                label: String::from("return"),
                detail: String::from("return expr"),
                replace_string_generator: Box::new(|name| format!("return {}", name)),
            },
            // foreach
            SnippetCompletionItem {
                label: String::from("for"),
                detail: String::from("forloop"),
                replace_string_generator: Box::new(|name| {
                    format!(
                        r#"for (let ${{1:i}} = 0, len = {}.length; ${{1:i}} < len; ${{1:i}}++) {{
  ${{0}}
}}"#,
                        name
                    )
                }),
            },
            SnippetCompletionItem {
                label: String::from("forof"),
                detail: String::from("forof"),
                replace_string_generator: Box::new(|name| {
                    format!(
                        r#"for (let ${{1:item}} of {}) {{
  ${{0}}
}}"#,
                        name
                    )
                }),
            },
            SnippetCompletionItem {
                label: String::from("foreach"),
                detail: String::from("expr.forEach(item => )"),
                replace_string_generator: Box::new(|name| {
                    format!(
                        r#"{}.forEach(${{1:item}} => {{
    ${{0}}
}})"#,
                        name
                    )
                }),
            },
        ];
        snippet_list
            .into_iter()
            .map(|snippet| {
                let mut item = CompletionItem::new_simple(snippet.label, snippet.detail);
                item.insert_text_format = Some(InsertTextFormat::Snippet);
                item.kind = Some(CompletionItemKind::Snippet);
                let replace_string = (snippet.replace_string_generator)(source_code.clone());
                item.documentation = Some(Documentation::String(replace_string.clone()));

                item.insert_text = Some(replace_string);
                item.additional_text_edits =
                    Some(vec![TextEdit::new(replace_range.clone(), "".into())]);
                item
            })
            .collect()
    }
}
