//! Round-trip serde tests for `AgentConversation` and the per-agent
//! `ViewSnapshot`. Pinning these guarantees `view/snapshot` carries
//! enough state for a fresh UI to seed itself end-to-end.

use loopal_protocol::{AgentEventPayload, AgentStatus, PermissionIntent, PermissionIntentRequest};
use loopal_view_state::ViewStateReducer;

#[test]
fn snapshot_round_trips_with_messages_and_streaming() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::Stream {
        text: "hello ".into(),
    });
    r.apply(AgentEventPayload::Stream {
        text: "world".into(),
    });
    r.apply(AgentEventPayload::AwaitingInput);

    let snap = r.snapshot();
    let json = serde_json::to_string(&snap).expect("serialize");
    let restored: loopal_view_state::ViewSnapshot =
        serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.rev, snap.rev);
    let conv = &restored.state.agent.conversation;
    assert_eq!(conv.messages.len(), 1);
    assert_eq!(conv.messages[0].role, "assistant");
    assert_eq!(conv.messages[0].content, "hello world");
}

#[test]
fn snapshot_round_trips_pending_permission_with_relay_id() {
    let mut r = ViewStateReducer::new("main");
    let request = PermissionIntentRequest::create(
        "logical-p1",
        "Bash",
        serde_json::json!({"command": "ls"}),
        serde_json::json!({"command": "ls"}),
        serde_json::json!({"type": "object"}),
        None,
    )
    .unwrap();
    let intent = PermissionIntent::bind(request.intent_seed, 7, 9, "p1").unwrap();
    let digest = intent.intent_digest();
    r.apply(AgentEventPayload::ToolPermissionRequest {
        id: "p1".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "ls"}),
        permission_intent: Some(Box::new(intent)),
    });

    let json = serde_json::to_string(&r.snapshot()).expect("serialize");
    let restored: loopal_view_state::ViewSnapshot =
        serde_json::from_str(&json).expect("deserialize");

    let perm = restored
        .state
        .agent
        .conversation
        .pending_permission
        .expect("pending_permission preserved");
    assert_eq!(perm.id, "p1");
    assert_eq!(perm.name, "Bash");
    assert_eq!(perm.intent_digest, Some(digest));
}

#[test]
fn snapshot_round_trips_completed_tool_call() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-1".into(),
        name: "Read".into(),
        input: serde_json::json!({"file_path": "/tmp/x"}),
    });
    r.apply(AgentEventPayload::ToolResult {
        id: "tc-1".into(),
        name: "Read".into(),
        result: "file contents".into(),
        is_error: false,
        duration_ms: Some(42),
        metadata: None,
    });

    let json = serde_json::to_string(&r.snapshot()).expect("serialize");
    let restored: loopal_view_state::ViewSnapshot =
        serde_json::from_str(&json).expect("deserialize");

    let tc = &restored.state.agent.conversation.messages[0].tool_calls[0];
    assert_eq!(tc.id.as_str(), "tc-1");
    assert_eq!(tc.name, "Read");
    let outcome = tc.state.outcome().expect("Done state expected");
    assert_eq!(outcome.content(), "file contents");
    assert!(!outcome.is_error());
}

#[test]
fn snapshot_preserves_observable_status() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::Running);

    let json = serde_json::to_string(&r.snapshot()).expect("serialize");
    let restored: loopal_view_state::ViewSnapshot =
        serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.state.agent.observable.status, AgentStatus::Running);
}

#[test]
fn snapshot_round_trips_stale_tool_call() {
    use loopal_tool_invocation::{StaleReason, ToolResultMetadata};
    use loopal_view_state::InvocationState;
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-stale".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "sleep 99"}),
    });
    r.apply(AgentEventPayload::ToolResult {
        id: "tc-stale".into(),
        name: "Bash".into(),
        result: "Watchdog timeout".into(),
        is_error: true,
        duration_ms: Some(330_000),
        metadata: Some(ToolResultMetadata::stale(StaleReason::WatchdogTimeout)),
    });

    let json = serde_json::to_string(&r.snapshot()).expect("serialize");
    let restored: loopal_view_state::ViewSnapshot =
        serde_json::from_str(&json).expect("deserialize");

    let tc = &restored.state.agent.conversation.messages[0].tool_calls[0];
    assert!(
        matches!(tc.state, InvocationState::Stale { .. }),
        "expected Stale, got {:?}",
        tc.state.variant_name()
    );
}

#[test]
fn snapshot_round_trips_cancelled_tool_call() {
    use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
    use loopal_view_state::InvocationState;
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-cancel".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "long-running"}),
    });
    r.apply(AgentEventPayload::ToolResult {
        id: "tc-cancel".into(),
        name: "Bash".into(),
        result: "Interrupted by user".into(),
        is_error: true,
        duration_ms: Some(1_200),
        metadata: Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
    });

    let json = serde_json::to_string(&r.snapshot()).expect("serialize");
    let restored: loopal_view_state::ViewSnapshot =
        serde_json::from_str(&json).expect("deserialize");

    let tc = &restored.state.agent.conversation.messages[0].tool_calls[0];
    assert!(
        matches!(tc.state, InvocationState::Cancelled { .. }),
        "expected Cancelled, got {:?}",
        tc.state.variant_name()
    );
}
