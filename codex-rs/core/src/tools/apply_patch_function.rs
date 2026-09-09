//! Structured `apply_patch` function-call translation for non-OpenAI providers.
//!
//! Muse and other Responses-compatible backends cannot send Codex freeform
//! custom tools. They call `apply_patch` as a function with either a canonical
//! patch in `input` or structured create/update/delete fields.

use std::path::Component;
use std::path::Path;

use crate::function_tool::FunctionCallError;
use serde_json::Value;

pub(crate) const APPLY_PATCH_TOOL_NAME: &str = "apply_patch";

const STRUCTURED_EXAMPLE: &str =
    r#"{"operation":"create_file","path":"notes.txt","content":"hello\n"}"#;

const CREATE_FILE: &str = "create_file";
const UPDATE_FILE: &str = "update_file";
const DELETE_FILE: &str = "delete_file";

pub(crate) fn apply_patch_function_description(original: Option<&str>) -> String {
    let base = original
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("Apply a file patch.");
    format!(
        "{base} Prefer structured fields: `operation` (`{CREATE_FILE}`, `{UPDATE_FILE}`, or `{DELETE_FILE}`), `path`, `content` ({CREATE_FILE}), `diff` ({UPDATE_FILE}), and optional `move_to`. You may instead pass a canonical apply_patch document in `input`."
    )
}

pub(crate) fn apply_patch_function_parameters() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "operation": {
                "type": "string",
                "enum": [CREATE_FILE, UPDATE_FILE, DELETE_FILE],
                "description": "Structured file operation. Omit when sending a canonical patch in `input`."
            },
            "path": {
                "type": "string",
                "description": "Target file path for a structured operation."
            },
            "content": {
                "type": "string",
                "description": "Full file contents for create_file."
            },
            "diff": {
                "type": "string",
                "description": "Unified diff hunks for update_file. May start with @@."
            },
            "move_to": {
                "type": "string",
                "description": "Optional new path when updating a file."
            },
            "input": {
                "type": "string",
                "description": "Canonical apply_patch document. Use this or the structured fields."
            }
        },
        "additionalProperties": false
    })
}

pub(crate) fn custom_input_from_function_arguments(
    arguments: &str,
) -> Result<Option<String>, FunctionCallError> {
    let value: Value = serde_json::from_str(arguments).map_err(|err| {
        structured_apply_patch_error(&format!("failed to parse apply_patch arguments: {err}"))
    })?;
    if value.get("operation").is_some() {
        return Ok(Some(canonical_patch_from_structured(&value)?));
    }
    if let Some(input) = value
        .get("input")
        .or_else(|| value.get("patch"))
        .or_else(|| value.get("command"))
        .and_then(Value::as_str)
    {
        return Ok(Some(input.to_string()));
    }
    Ok(value.as_str().map(ToString::to_string))
}

fn canonical_patch_from_structured(value: &Value) -> Result<String, FunctionCallError> {
    let operation = required_string(value, "operation")?;
    let path = required_string(value, "path")?;
    validate_apply_patch_path(&path)?;
    match operation.as_str() {
        CREATE_FILE => {
            let content = optional_string(value, "content").unwrap_or_default();
            Ok(create_file_patch(&path, &content))
        }
        UPDATE_FILE => {
            let diff = required_string(value, "diff")?;
            let move_to = optional_string(value, "move_to");
            if let Some(move_to) = move_to.as_deref() {
                validate_apply_patch_path(move_to)?;
            }
            Ok(update_file_patch(&path, &diff, move_to.as_deref()))
        }
        DELETE_FILE => Ok(delete_file_patch(&path)),
        other => Err(structured_apply_patch_error(&format!(
            "unsupported apply_patch operation `{other}`; use `{CREATE_FILE}`, `{UPDATE_FILE}`, or `{DELETE_FILE}`"
        ))),
    }
}

fn create_file_patch(path: &str, content: &str) -> String {
    let mut patch = format!("*** Begin Patch\n*** Add File: {path}");
    let added = prefix_added_lines(content);
    if !added.is_empty() {
        patch.push('\n');
        patch.push_str(&added);
    }
    patch.push_str("\n*** End Patch");
    patch
}

fn update_file_patch(path: &str, diff: &str, move_to: Option<&str>) -> String {
    let mut patch = format!("*** Begin Patch\n*** Update File: {path}");
    if let Some(move_to) = move_to {
        patch.push_str(&format!("\n*** Move to: {move_to}"));
    }
    let hunks = normalize_update_diff(diff);
    if !hunks.is_empty() {
        patch.push('\n');
        patch.push_str(&hunks);
    }
    patch.push_str("\n*** End Patch");
    patch
}

fn delete_file_patch(path: &str) -> String {
    format!("*** Begin Patch\n*** Delete File: {path}\n*** End Patch")
}

fn normalize_update_diff(diff: &str) -> String {
    let trimmed = diff.trim_start_matches('\n').trim_end_matches('\n');
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("@@") || trimmed.contains("*** ") {
        return trimmed.to_string();
    }
    format!("@@\n{trimmed}")
}

fn prefix_added_lines(content: &str) -> String {
    let stripped = content.strip_suffix('\n').unwrap_or(content);
    if stripped.is_empty() {
        return String::new();
    }
    stripped
        .split('\n')
        .map(|line| format!("+{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn validate_apply_patch_path(path: &str) -> Result<(), FunctionCallError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(structured_apply_patch_error(
            "apply_patch `path` must be a non-empty file path",
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(structured_apply_patch_error(
            "apply_patch `path` cannot contain control characters",
        ));
    }
    if Path::new(trimmed)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(structured_apply_patch_error(
            "apply_patch `path` cannot contain `..` components",
        ));
    }
    Ok(())
}

fn required_string(value: &Value, field: &str) -> Result<String, FunctionCallError> {
    match value.get(field).and_then(Value::as_str) {
        Some(text) => Ok(text.trim().to_string()),
        None => Err(structured_apply_patch_error(&format!(
            "apply_patch `{field}` must be a string"
        ))),
    }
}

fn optional_string(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn structured_apply_patch_error(reason: &str) -> FunctionCallError {
    FunctionCallError::RespondToModel(format!("{reason}\n\nExample:\n{STRUCTURED_EXAMPLE}"))
}

#[cfg(test)]
#[path = "apply_patch_function_tests.rs"]
mod tests;
