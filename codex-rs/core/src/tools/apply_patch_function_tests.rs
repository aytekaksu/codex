use super::*;
use pretty_assertions::assert_eq;

#[test]
fn structured_create_file_becomes_canonical_add_patch() {
    let arguments = serde_json::json!({
        "operation": "create_file",
        "path": "notes.txt",
        "content": "hello\nworld\n"
    })
    .to_string();

    assert_eq!(
        custom_input_from_function_arguments(&arguments).expect("create should translate"),
        Some("*** Begin Patch\n*** Add File: notes.txt\n+hello\n+world\n*** End Patch".to_string())
    );
}

#[test]
fn structured_update_file_wraps_diff_and_optional_move() {
    let arguments = serde_json::json!({
        "operation": "update_file",
        "path": "src/lib.rs",
        "move_to": "src/main.rs",
        "diff": "-old\n+new"
    })
    .to_string();

    assert_eq!(
        custom_input_from_function_arguments(&arguments).expect("update should translate"),
        Some(
            "*** Begin Patch\n*** Update File: src/lib.rs\n*** Move to: src/main.rs\n@@\n-old\n+new\n*** End Patch"
                .to_string()
        )
    );
}

#[test]
fn structured_delete_file_becomes_canonical_delete_patch() {
    let arguments = serde_json::json!({
        "operation": "delete_file",
        "path": "obsolete.txt"
    })
    .to_string();

    assert_eq!(
        custom_input_from_function_arguments(&arguments).expect("delete should translate"),
        Some("*** Begin Patch\n*** Delete File: obsolete.txt\n*** End Patch".to_string())
    );
}

#[test]
fn input_fallback_keeps_canonical_patch() {
    let patch = "*** Begin Patch\n*** Add File: a.txt\n+hi\n*** End Patch";
    let arguments = serde_json::json!({ "input": patch }).to_string();

    assert_eq!(
        custom_input_from_function_arguments(&arguments).expect("input fallback"),
        Some(patch.to_string())
    );
}

#[test]
fn invalid_parent_path_includes_corrected_example() {
    let arguments = serde_json::json!({
        "operation": "create_file",
        "path": "../secret.txt",
        "content": "nope"
    })
    .to_string();

    let err =
        custom_input_from_function_arguments(&arguments).expect_err("parent-dir paths should fail");
    let FunctionCallError::RespondToModel(message) = err else {
        panic!("expected model-facing path error");
    };
    assert!(message.contains("`..`"));
    assert!(message.contains(STRUCTURED_EXAMPLE));
}

#[test]
fn empty_path_includes_corrected_example() {
    let arguments = serde_json::json!({
        "operation": "delete_file",
        "path": "   "
    })
    .to_string();

    let err = custom_input_from_function_arguments(&arguments).expect_err("empty path should fail");
    let FunctionCallError::RespondToModel(message) = err else {
        panic!("expected model-facing path error");
    };
    assert!(message.contains("non-empty"));
    assert!(message.contains(STRUCTURED_EXAMPLE));
}
