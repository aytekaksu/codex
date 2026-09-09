//! Bounded Muse child guidance listing the exact sanitized wire tool names.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseItem;
use serde_json::Value;

const MAX_TOOL_NAMES: usize = 64;
const MAX_BODY_CHARS: usize = 4_000;

/// Developer fragment that lists the exact tools a Muse child may call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MuseToolGuidance {
    body: String,
}

impl MuseToolGuidance {
    pub(crate) fn new(tool_names: &[String]) -> Self {
        Self {
            body: render_muse_tool_guidance(tool_names),
        }
    }

    pub(crate) fn matches_item(item: &ResponseItem) -> bool {
        let ResponseItem::Message { content, .. } = item else {
            return false;
        };
        let (open, _) = Self::type_markers();
        content
            .iter()
            .any(|item| matches!(item, ContentItem::InputText { text } if text.contains(open)))
    }
}

impl ContextualUserFragment for MuseToolGuidance {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("muse.tool_guidance".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn requires_separate_message(&self) -> bool {
        true
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<muse_tool_guidance>", "</muse_tool_guidance>")
    }

    fn body(&self) -> String {
        self.body.clone()
    }
}

pub(crate) fn collect_wire_tool_names(value: &Value) -> Vec<String> {
    let mut names = Vec::new();
    collect_wire_tool_names_inner(value, &mut names);
    names.sort();
    names.dedup();
    names.truncate(MAX_TOOL_NAMES);
    names
}

pub(crate) fn upsert_muse_tool_guidance(input: &mut Vec<ResponseItem>, tool_names: &[String]) {
    let next = ContextualUserFragment::into(MuseToolGuidance::new(tool_names));
    if let Some(existing) = input
        .iter_mut()
        .find(|item| MuseToolGuidance::matches_item(item))
    {
        *existing = next;
        return;
    }
    input.push(next);
}

fn collect_wire_tool_names_inner(value: &Value, names: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_wire_tool_names_inner(item, names);
            }
        }
        Value::Object(map) => {
            match map.get("type").and_then(Value::as_str) {
                Some("web_search") => names.push("web_search".to_string()),
                Some("function" | "custom") => {
                    if let Some(name) = map.get("name").and_then(Value::as_str) {
                        let name = name.trim();
                        if !name.is_empty() {
                            names.push(name.to_string());
                        }
                    }
                }
                Some("namespace") => {
                    if let Some(tools) = map.get("tools") {
                        collect_wire_tool_names_inner(tools, names);
                    }
                    return;
                }
                _ => {}
            }
            for child in map.values() {
                collect_wire_tool_names_inner(child, names);
            }
        }
        _ => {}
    }
}

fn render_muse_tool_guidance(tool_names: &[String]) -> String {
    let mut body = String::from("Available tools for this turn (use only these exact names):\n");
    if tool_names.is_empty() {
        body.push_str("- (none advertised)\n");
    } else {
        for name in tool_names.iter().take(MAX_TOOL_NAMES) {
            body.push_str("- ");
            body.push_str(name);
            body.push('\n');
        }
    }
    if tool_names.iter().any(|name| name == "apply_patch") {
        body.push_str(
            "\napply_patch accepts structured create_file/update_file/delete_file fields or a canonical patch in `input`.\n",
        );
    }
    body.push_str(
        "Do not invent dotted, MCP, REPL, or code-mode names unless they appear in this list.",
    );
    if body.chars().count() > MAX_BODY_CHARS {
        body = body.chars().take(MAX_BODY_CHARS).collect();
    }
    body
}

#[cfg(test)]
#[path = "muse_tool_guidance_tests.rs"]
mod tests;
