use anyhow::Result;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_web_search_call_done;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

const CITATION_URL: &str = "https://example.com/muse-search-citation";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_turn_search_keeps_citations_without_previous_response_id() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_web_search_call_done("ws-1", "completed", "muse spark docs"),
            ev_assistant_message("msg-1", &format!("Found the docs at {CITATION_URL}")),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-2", "still valid"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_model("gpt-5.4");
    let test = builder.build(&server).await?;
    test.submit_turn("search for muse spark docs").await?;
    test.submit_turn("use those citations").await?;

    let second = second_mock.single_request();
    let body = second.body_json();
    assert_eq!(body.get("store"), Some(&serde_json::json!(false)));
    assert!(body.get("previous_response_id").is_none());
    assert!(
        body.to_string().contains(CITATION_URL),
        "next-turn history should keep URL citations from the search turn"
    );

    Ok(())
}
