use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_tool_invocation::{CancelCause, StaleReason, ToolResultMetadata};

#[test]
fn tool_result_stale_metadata_round_trips_over_wire() {
    let event = AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc_stale".into(),
        name: "Bash".into(),
        result: "Watchdog timeout".into(),
        is_error: true,
        duration_ms: Some(330_000),
        metadata: Some(ToolResultMetadata::stale(StaleReason::WatchdogTimeout)),
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    let AgentEventPayload::ToolResult { metadata, .. } = deserialized.payload else {
        panic!("expected ToolResult");
    };
    match metadata.expect("metadata must round-trip") {
        ToolResultMetadata::Stale { reason } => {
            assert_eq!(reason, StaleReason::WatchdogTimeout);
        }
        other => panic!("expected Stale, got {other:?}"),
    }
}

#[test]
fn tool_result_cancel_metadata_round_trips_over_wire() {
    let event = AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc_cancel".into(),
        name: "Bash".into(),
        result: "Interrupted by user".into(),
        is_error: true,
        duration_ms: Some(1_200),
        metadata: Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    let AgentEventPayload::ToolResult { metadata, .. } = deserialized.payload else {
        panic!("expected ToolResult");
    };
    match metadata.expect("metadata must round-trip") {
        ToolResultMetadata::Cancelled { cause } => {
            assert_eq!(cause, CancelCause::UserInterrupt);
        }
        other => panic!("expected Cancelled, got {other:?}"),
    }
}

#[test]
fn tool_result_no_metadata_serializes_without_field() {
    let event = AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc_plain".into(),
        name: "Read".into(),
        result: "content".into(),
        is_error: false,
        duration_ms: Some(5),
        metadata: None,
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    let AgentEventPayload::ToolResult { metadata, .. } = deserialized.payload else {
        panic!("expected ToolResult");
    };
    assert!(metadata.is_none());
}
