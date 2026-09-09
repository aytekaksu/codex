use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn collect_wire_tool_names_uses_exact_function_and_hosted_names() {
    let tools = json!([
        { "type": "function", "name": "exec_command" },
        { "type": "function", "name": "apply_patch" },
        { "type": "web_search" },
        {
            "type": "namespace",
            "name": "collaboration",
            "tools": [{ "type": "function", "name": "spawn_agent" }]
        }
    ]);

    assert_eq!(
        collect_wire_tool_names(&tools),
        vec![
            "apply_patch".to_string(),
            "exec_command".to_string(),
            "spawn_agent".to_string(),
            "web_search".to_string()
        ]
    );
}

#[test]
fn upsert_replaces_previous_guidance_in_place() {
    let mut input = Vec::new();
    upsert_muse_tool_guidance(&mut input, &["exec_command".to_string()]);
    upsert_muse_tool_guidance(
        &mut input,
        &["apply_patch".to_string(), "exec_command".to_string()],
    );

    assert_eq!(input.len(), 1);
    let ResponseItem::Message { content, .. } = &input[0] else {
        panic!("expected developer message");
    };
    let ContentItem::InputText { text } = &content[0] else {
        panic!("expected input text");
    };
    assert!(text.contains("<muse_tool_guidance>"));
    assert!(text.contains("- apply_patch"));
    assert!(text.contains("structured create_file"));
    assert!(!text.contains("collaboration.spawn_agent"));
}
