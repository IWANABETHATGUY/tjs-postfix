use lsp_text_document::FullTextDocument;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;
use tower_lsp::{lsp_types::*, Client};
use tree_sitter::{Node, Parser, Tree};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PostfixTemplate {
    snippet_key: String,
    code: String,
}

pub struct SnippetCompletionItem {
    label: String,
    detail: String,
    replace_string_generator: Box<dyn Fn(String) -> String>,
}

pub struct Backend {
    pub(crate) client: Client,
    pub(crate) document_map: Mutex<HashMap<String, FullTextDocument>>,
    pub(crate) parser: Mutex<Parser>,
    pub(crate) parse_tree_map: Mutex<HashMap<String, Tree>>,
    postfix_template_list: Arc<StdMutex<Vec<PostfixTemplate>>>,
    pub workspace_folder: Mutex<Vec<WorkspaceFolder>>,
}
impl Backend {
    pub fn new(
        client: Client,
        document_map: Mutex<HashMap<String, FullTextDocument>>,
        parser: Mutex<Parser>,
        postfix_template_list: Arc<StdMutex<Vec<PostfixTemplate>>>,
        parse_tree_map: Mutex<HashMap<String, Tree>>,
    ) -> Self {
        Self {
            client,
            document_map,
            parser,
            postfix_template_list,
            parse_tree_map,
            workspace_folder: Mutex::new(vec![]),
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
        source_code: &str,
        replace_range: &Range,
    ) -> Vec<CompletionItem> {
        if let Ok(template_list) = self.postfix_template_list.lock() {
            template_list
                .iter()
                .map(|template_item| {
                    let mut item = CompletionItem::new_simple(
                        template_item.snippet_key.clone(),
                        template_item.code.clone(),
                    );
                    item.kind = Some(CompletionItemKind::SNIPPET);
                    item.insert_text_format = Some(InsertTextFormat::SNIPPET);
                    let replace_string = template_item.code.replace("$$", source_code);
                    item.documentation = Some(Documentation::String(replace_string.clone()));
                    item.text_edit = Some(CompletionTextEdit::Edit(TextEdit::new(
                        replace_range.clone(),
                        replace_string.clone(),
                    )));
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
        source_code: &str,
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
                label: String::from("ifn"),
                detail: String::from("if (!expr)"),
                replace_string_generator: Box::new(|name| {
                    format!(
                        r#"if (!{}) {{
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
                label: String::from("call"),
                detail: String::from("call(expr)"),
                replace_string_generator: Box::new(|name| format!("${{0}}({})", name)),
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
                item.insert_text_format = Some(InsertTextFormat::SNIPPET);
                item.kind = Some(CompletionItemKind::SNIPPET);
                let replace_string = (snippet.replace_string_generator)(source_code.to_string());
                item.documentation = Some(Documentation::String(replace_string.clone()));

                item.insert_text = Some(replace_string);
                item.additional_text_edits =
                    Some(vec![TextEdit::new(replace_range.clone(), "".into())]);
                item
            })
            .collect()
    }
}

pub struct TreeWrapper(pub Tree);
impl std::fmt::Display for TreeWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        pretty_display(f, self.0.root_node())?;
        Ok(())
    }
}

pub fn pretty_display(f: &mut std::fmt::Formatter<'_>, root: Node) -> std::fmt::Result {
    let mut stack = Vec::new();
    if !root.is_named() {
        return Ok(());
    }
    stack.push((root, 0));
    while let Some((node, level)) = stack.pop() {
        let kind = node.kind();
        let start = node.start_position();
        let end = node.end_position();
        writeln!(
            f,
            "{}{} [{}, {}] - [{}, {}] ",
            " ".repeat(level * 2),
            kind,
            start.row,
            start.column,
            end.row,
            end.column
        )?;
        for i in (0..node.named_child_count()).rev() {
            let child = node.named_child(i).unwrap();
            stack.push((child, level + 1));
        }
    }
    Ok(())
}
