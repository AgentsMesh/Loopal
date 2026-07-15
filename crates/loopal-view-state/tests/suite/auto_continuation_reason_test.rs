use loopal_protocol::AgentEventPayload;
use loopal_view_state::ViewStateReducer;

fn message_for(reason: &str) -> String {
    let mut reducer = ViewStateReducer::new("root");
    reducer.apply(AgentEventPayload::AutoContinuation {
        continuation: 1,
        max_continuations: 3,
        reason: reason.into(),
    });
    reducer.state().agent.conversation.messages[0]
        .content
        .clone()
}

#[test]
fn continuation_reason_selects_truthful_message() {
    assert_eq!(
        message_for("max_tokens_without_tools"),
        "Output truncated (max_tokens). Auto-continuing (1/3)"
    );
    assert_eq!(
        message_for("max_tokens_with_tools"),
        "Output truncated during tool calls (max_tokens); incomplete tools discarded. Auto-continuing (1/3)"
    );
    assert_eq!(
        message_for("pause_turn"),
        "Provider paused the turn. Auto-continuing (1/3)"
    );
    assert_eq!(
        message_for("stream_truncated"),
        "Response stream ended unexpectedly. Auto-continuing (1/3)"
    );
}

#[test]
fn missing_legacy_reason_keeps_max_tokens_fallback() {
    assert!(message_for("").starts_with("Output truncated (max_tokens)"));
}
