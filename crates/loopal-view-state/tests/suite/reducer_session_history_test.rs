use loopal_protocol::{
    AgentEventPayload, ProjectedMessage, ProjectedToolCall, SessionHistorySnapshot,
};
use loopal_view_state::{InvocationState, ViewStateReducer};
use std::time::Duration;

fn message(content: &str, with_tool: bool) -> ProjectedMessage {
    ProjectedMessage {
        role: "assistant".into(),
        content: content.into(),
        tool_calls: with_tool
            .then(|| ProjectedToolCall {
                id: "tool-1".into(),
                name: "Read".into(),
                summary: "Read(file)".into(),
                result: Some("contents".into()),
                is_error: false,
                input: Some(serde_json::json!({"path": "file"})),
                metadata: None,
            })
            .into_iter()
            .collect(),
        image_count: 0,
        skill_info: None,
    }
}

#[test]
fn history_snapshot_authoritatively_replaces_conversation() {
    let mut reducer = ViewStateReducer::new("main");
    reducer.apply(AgentEventPayload::Running);
    reducer.apply(AgentEventPayload::Stream {
        text: "stale live text".into(),
    });
    reducer.apply(AgentEventPayload::SessionHistoryLoaded(
        SessionHistorySnapshot {
            session_id: "session-1".into(),
            messages: vec![message("persisted answer", true)],
            truncated: true,
        },
    ));

    let conversation = &reducer.state().agent.conversation;
    assert_eq!(
        reducer.state().agent.session_id.as_deref(),
        Some("session-1")
    );
    assert_eq!(conversation.messages.len(), 1);
    assert_eq!(conversation.messages[0].content, "persisted answer");
    assert!(conversation.streaming_text.is_empty());
    assert!(conversation.history_truncated);
    assert!(conversation.is_recently_active(Duration::from_secs(1)));
    assert!(matches!(
        conversation.messages[0].tool_calls[0].state,
        InvocationState::Done { .. }
    ));

    reducer.apply(AgentEventPayload::SessionHistoryLoaded(
        SessionHistorySnapshot {
            session_id: "session-2".into(),
            messages: vec![message("new authority", false)],
            truncated: false,
        },
    ));
    let conversation = &reducer.state().agent.conversation;
    assert_eq!(conversation.messages[0].content, "new authority");
    assert!(!conversation.history_truncated);

    let encoded = serde_json::to_string(&reducer.snapshot()).unwrap();
    let restored: loopal_view_state::ViewSnapshot = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        restored.state.agent.conversation.messages[0].content,
        "new authority"
    );
    assert!(!restored.state.agent.conversation.history_truncated);
}
