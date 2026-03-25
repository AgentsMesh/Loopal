//! Reusable test assertions for events, buffers, and JSON-RPC responses.

use loopal_protocol::AgentEventPayload;

/// Panic if no event matches the predicate.
pub fn assert_event_has(
    events: &[AgentEventPayload],
    predicate: impl Fn(&AgentEventPayload) -> bool,
    msg: &str,
) {
    assert!(events.iter().any(&predicate), "{msg}: {events:?}");
}

/// Assert events contain at least one `Stream` payload.
pub fn assert_has_stream(events: &[AgentEventPayload]) {
    assert_event_has(
        events,
        |e| matches!(e, AgentEventPayload::Stream { .. }),
        "expected Stream event",
    );
}

/// Assert events contain a `ToolCall` for the named tool.
pub fn assert_has_tool_call(events: &[AgentEventPayload], name: &str) {
    assert_event_has(
        events,
        |e| matches!(e, AgentEventPayload::ToolCall { name: n, .. } if n == name),
        &format!("expected ToolCall({name})"),
    );
}

/// Assert events contain a `Finished` payload.
pub fn assert_has_finished(events: &[AgentEventPayload]) {
    assert_event_has(
        events,
        |e| matches!(e, AgentEventPayload::Finished),
        "expected Finished event",
    );
}

/// Assert events contain a `ThinkingStream` payload.
pub fn assert_has_thinking(events: &[AgentEventPayload]) {
    assert_event_has(
        events,
        |e| matches!(e, AgentEventPayload::ThinkingStream { .. }),
        "expected ThinkingStream event",
    );
}

/// Assert a rendered buffer string contains the expected text.
pub fn assert_buffer_contains(buffer: &str, expected: &str) {
    assert!(
        buffer.contains(expected),
        "expected buffer to contain {expected:?}, got:\n{buffer}"
    );
}

/// Assert a JSON-RPC response has no error field.
pub fn assert_json_rpc_ok(response: &serde_json::Value) {
    assert!(
        response.get("error").is_none(),
        "expected success, got error: {response}"
    );
}

/// Assert a JSON-RPC response has an error with the given code.
pub fn assert_json_rpc_error(response: &serde_json::Value, code: i64) {
    let err = response.get("error").expect("expected error in response");
    assert_eq!(
        err["code"].as_i64().unwrap(),
        code,
        "wrong error code in: {response}"
    );
}

pub fn assert_has_error(events: &[AgentEventPayload]) {
    assert_event_has(
        events,
        |e| matches!(e, AgentEventPayload::Error { .. }),
        "expected Error event",
    );
}

pub fn assert_has_max_turns(events: &[AgentEventPayload]) {
    assert_event_has(
        events,
        |e| matches!(e, AgentEventPayload::MaxTurnsReached { .. }),
        "expected MaxTurnsReached event",
    );
}

pub fn assert_has_tool_result(events: &[AgentEventPayload], tool_name: &str, expect_error: bool) {
    assert_event_has(
        events,
        |e| matches!(e, AgentEventPayload::ToolResult { name, is_error, .. } if name == tool_name && *is_error == expect_error),
        &format!("expected ToolResult({tool_name}, is_error={expect_error})"),
    );
}

pub fn assert_has_mode_changed(events: &[AgentEventPayload], expected_mode: &str) {
    assert_event_has(
        events,
        |e| matches!(e, AgentEventPayload::ModeChanged { mode } if mode == expected_mode),
        &format!("expected ModeChanged({expected_mode})"),
    );
}
