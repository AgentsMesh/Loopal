//! IPC acceptance tests — full-stack: Client → IPC → Server(agent_loop) → IPC → Bridge.
//!
//! These tests exercise the actual agent loop with mock LLM responses,
//! verifying the complete multi-process protocol stack end-to-end.

use loopal_test_support::assertions;
use loopal_test_support::ipc_harness::{build_ipc_harness, collect_ipc_events};
use loopal_test_support::scenarios;

/// Simple text response: agent streams "Hello!" back through IPC.
#[tokio::test]
async fn acceptance_simple_text_response() {
    let mut harness = build_ipc_harness(scenarios::simple_text("Hello from IPC!")).await;
    let events = collect_ipc_events(&mut harness.event_rx).await;

    assertions::assert_has_stream(&events);
    let text = loopal_test_support::events::extract_texts(&events);
    assert!(
        text.contains("Hello from IPC!"),
        "expected 'Hello from IPC!' in: {text}"
    );
}

/// Agent processes prompt and finishes (non-interactive mode).
#[tokio::test]
async fn acceptance_non_interactive_completes() {
    let mut harness = build_ipc_harness(scenarios::two_turn("First reply", "Second reply")).await;
    let events = collect_ipc_events(&mut harness.event_rx).await;

    assertions::assert_has_stream(&events);
    assertions::assert_has_finished(&events);
    let text = loopal_test_support::events::extract_texts(&events);
    assert!(
        text.contains("First reply"),
        "expected first reply in: {text}"
    );
}

/// Tool execution: agent calls Read tool → executes → returns result.
#[tokio::test]
async fn acceptance_tool_execution() {
    let mut harness = build_ipc_harness(scenarios::tool_then_text(
        "tc-1",
        "Read",
        serde_json::json!({"file_path": "/nonexistent/test.txt"}),
        "Read completed",
    ))
    .await;

    let events = collect_ipc_events(&mut harness.event_rx).await;
    assertions::assert_has_tool_call(&events, "Read");
}
