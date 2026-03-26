//! E2E tests for error handling: provider errors, missing files, unknown tools.

use loopal_test_support::{assertions, chunks};

use super::e2e_harness::build_tui_harness;

#[tokio::test]
async fn test_provider_error_mid_stream() {
    // Provider returns partial text then errors out
    let calls = vec![vec![
        chunks::text("partial response"),
        chunks::provider_error("connection timeout"),
    ]];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let events = harness.collect_until_idle().await;

    assertions::assert_has_error(&events);
}

#[tokio::test]
async fn test_provider_error_only() {
    // Provider errors immediately — no text at all
    let calls = vec![vec![chunks::provider_error("service unavailable")]];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let events = harness.collect_until_idle().await;

    assertions::assert_has_error(&events);
    assertions::assert_has_finished(&events);
}

#[tokio::test]
async fn test_read_nonexistent_file() {
    let calls = vec![
        chunks::tool_turn(
            "tc-1",
            "Read",
            serde_json::json!({"file_path": "/nonexistent/path/file.txt"}),
        ),
        chunks::text_turn("The file was not found."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let events = harness.collect_until_idle().await;

    // Read should return an error result (file not found)
    assertions::assert_has_tool_result(&events, "Read", true);
    // Agent should continue and produce text after the error
    assertions::assert_has_stream(&events);
}

#[tokio::test]
async fn test_unknown_tool_call() {
    let calls = vec![
        chunks::tool_turn("tc-1", "NonExistentTool", serde_json::json!({})),
        chunks::text_turn("I couldn't use that tool."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let events = harness.collect_until_idle().await;

    // Unknown tool → error result
    assertions::assert_has_tool_result(&events, "NonExistentTool", true);
    assertions::assert_has_stream(&events);
}

#[tokio::test]
async fn test_malformed_tool_input() {
    // Tool input is a string instead of an object — Read expects an object
    let calls = vec![
        chunks::tool_turn("tc-1", "Read", serde_json::json!("not an object")),
        chunks::text_turn("Error occurred."),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let events = harness.collect_until_idle().await;

    assertions::assert_has_tool_result(&events, "Read", true);
    assertions::assert_has_stream(&events);
}

#[tokio::test]
async fn test_error_event_text_captured() {
    let calls = vec![vec![chunks::provider_error("api key expired")]];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let events = harness.collect_until_idle().await;

    let error_msgs: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            loopal_protocol::AgentEventPayload::Error { message } => Some(message.as_str()),
            _ => None,
        })
        .collect();
    assert!(!error_msgs.is_empty(), "should have error events");
}
