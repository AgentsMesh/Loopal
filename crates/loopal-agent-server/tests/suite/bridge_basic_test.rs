//! Tests for parent-child agent IPC interaction: basic scenarios.
//! Simulates sub-agent server via in-memory duplex + mock provider.

use loopal_protocol::AgentEventPayload;
use loopal_test_support::scenarios;

use super::bridge_helpers::{collect_agent_events, init_and_start, start_child_server};

// ── Tests ────────────────────────────────────────────────────────────

/// Sub-agent returns text -> parent sees Stream events + Finished.
#[tokio::test]
async fn child_text_streamed_and_finished() {
    let (conn, mut rx, fixture, _join) =
        start_child_server(scenarios::simple_text("sub-agent output")).await;
    let _sid = init_and_start(&conn, &fixture, "hello").await;

    let events = collect_agent_events(&mut rx).await;
    assert!(!events.is_empty(), "should receive events");

    let has_stream = events
        .iter()
        .any(|e| matches!(e, AgentEventPayload::Stream { .. }));
    let has_finished = events
        .iter()
        .any(|e| matches!(e, AgentEventPayload::Finished));
    assert!(has_stream, "should have Stream event");
    assert!(has_finished, "should have Finished event");

    // Accumulate stream text
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::Stream { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        text.contains("sub-agent output"),
        "stream text should contain response"
    );
}

/// Sub-agent calls a tool -> parent sees ToolCall + ToolResult events.
#[tokio::test]
async fn child_tool_call_events_visible() {
    let calls = scenarios::tool_then_text(
        "tc-1",
        "Glob",
        serde_json::json!({"pattern": "*.rs"}),
        "found files",
    );
    let (conn, mut rx, fixture, _join) = start_child_server(calls).await;
    let _sid = init_and_start(&conn, &fixture, "find rust files").await;

    let events = collect_agent_events(&mut rx).await;

    let has_tool_call = events
        .iter()
        .any(|e| matches!(e, AgentEventPayload::ToolCall { name, .. } if name == "Glob"));
    let has_tool_result = events
        .iter()
        .any(|e| matches!(e, AgentEventPayload::ToolResult { name, .. } if name == "Glob"));
    let has_finished = events
        .iter()
        .any(|e| matches!(e, AgentEventPayload::Finished));

    assert!(has_tool_call, "should see ToolCall for Glob");
    assert!(has_tool_result, "should see ToolResult for Glob");
    assert!(has_finished, "should finish");
}

/// Sub-agent runs multiple tool turns -> all events visible -> final text.
#[tokio::test]
async fn child_multi_turn_tool_chain() {
    let calls = scenarios::sequential_tools(
        &[
            ("tc-ls", "Ls", serde_json::json!({"path": "."})),
            ("tc-glob", "Glob", serde_json::json!({"pattern": "*.toml"})),
        ],
        "analysis complete",
    );
    let (conn, mut rx, fixture, _join) = start_child_server(calls).await;
    let _sid = init_and_start(&conn, &fixture, "analyze project").await;

    let events = collect_agent_events(&mut rx).await;

    let tool_names: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::ToolCall { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert!(tool_names.contains(&"Ls"), "should call Ls");
    assert!(tool_names.contains(&"Glob"), "should call Glob");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEventPayload::Finished)),
        "should finish"
    );
}

/// Finished event arrives -> test completes within timeout (no hang regression).
#[tokio::test]
async fn child_finished_no_hang() {
    let (conn, mut rx, fixture, _join) = start_child_server(scenarios::simple_text("final")).await;
    let _sid = init_and_start(&conn, &fixture, "quick task").await;

    // This must complete within T (10s). If bridge_child_events didn't exit
    // on Finished, this would hang forever.
    let result =
        tokio::time::timeout(super::bridge_helpers::T, collect_agent_events(&mut rx)).await;
    assert!(result.is_ok(), "should not hang after Finished");

    let events = result.unwrap();
    let last = events.last().expect("should have events");
    assert!(
        matches!(last, AgentEventPayload::Finished),
        "last event should be Finished"
    );
}
