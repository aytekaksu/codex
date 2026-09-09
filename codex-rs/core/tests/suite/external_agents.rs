use anyhow::Result;
use codex_core::config::AgentRoleConfig;
use codex_features::Feature;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use serde_json::json;
use std::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;

const SPAWN_CALL_ID: &str = "external-spawn-1";
const TURN_1_PROMPT: &str = "spawn a muse worker";
const MUSE_TASK: &str = "exact plaintext muse task marker";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_agents_spawn_sends_plaintext_user_message_to_muse_child() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let spawn_args = serde_json::to_string(&json!({
        "message": MUSE_TASK,
        "task_name": "muse_inventory",
    }))?;
    let mut spawn_event = ev_function_call_with_namespace(
        SPAWN_CALL_ID,
        "external_agents",
        "spawn_agent",
        &spawn_args,
    );
    spawn_event["item"]["encrypted_function_args"] = json!([]);
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-parent-1"),
            spawn_event,
            ev_completed("resp-parent-1"),
        ]),
    )
    .await;
    let child_request_log = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| is_muse_child_request(req),
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-parent-2"),
            ev_assistant_message("msg-parent-2", "done"),
            ev_completed("resp-parent-2"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_model("gpt-5.6-sol")
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
            config.agent_roles.insert(
                "muse_spark".to_string(),
                AgentRoleConfig {
                    description: Some("Muse Spark worker".to_string()),
                    config_file: None,
                    nickname_candidates: None,
                },
            );
        });
    let test = builder.build(&server).await?;
    test.submit_turn(TURN_1_PROMPT).await?;

    let deadline = Instant::now() + Duration::from_secs(2);
    let child_request = loop {
        if let Some(request) = child_request_log
            .requests()
            .into_iter()
            .find(|request| request_has_plaintext_user_task(request, MUSE_TASK))
        {
            break request;
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for Muse child plaintext request");
        }
        sleep(Duration::from_millis(10)).await;
    };

    assert!(
        child_request.inputs_of_type("agent_message").is_empty(),
        "Muse children must not receive encrypted agent_message items"
    );
    assert!(
        request_has_plaintext_user_task(&child_request, MUSE_TASK),
        "Muse child should see the exact plaintext user message: {:?}",
        child_request.inputs_of_type("message")
    );
    assert!(
        !input_items_include_encrypted_content(&child_request),
        "Muse child input items must not include encrypted_content payloads"
    );

    Ok(())
}

fn is_muse_child_request(req: &wiremock::Request) -> bool {
    body_contains(req, MUSE_TASK)
        && !body_contains(req, SPAWN_CALL_ID)
        && !body_contains(req, TURN_1_PROMPT)
}

fn request_has_plaintext_user_task(
    request: &core_test_support::responses::ResponsesRequest,
    task: &str,
) -> bool {
    request.inputs_of_type("message").iter().any(|item| {
        item["role"].as_str() == Some("user")
            && item.to_string().contains(task)
            && !item_has_encrypted_content(item)
    })
}

fn input_items_include_encrypted_content(
    request: &core_test_support::responses::ResponsesRequest,
) -> bool {
    request
        .body_json()
        .get("input")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| items.iter().any(item_has_encrypted_content))
}

fn item_has_encrypted_content(item: &serde_json::Value) -> bool {
    item.get("encrypted_content").is_some()
        || item["content"].as_array().is_some_and(|content| {
            content.iter().any(|part| {
                part.get("type").and_then(serde_json::Value::as_str) == Some("encrypted_content")
            })
        })
}

fn body_contains(req: &wiremock::Request, text: &str) -> bool {
    decoded_body(req)
        .and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
}

fn decoded_body(req: &wiremock::Request) -> Option<Vec<u8>> {
    let is_zstd = req
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
        });
    if is_zstd {
        zstd::stream::decode_all(std::io::Cursor::new(&req.body)).ok()
    } else {
        Some(req.body.clone())
    }
}
