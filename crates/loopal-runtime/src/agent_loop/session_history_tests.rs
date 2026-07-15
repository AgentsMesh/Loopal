use loopal_protocol::{AgentEventPayload, ProjectedMessage};

use super::{
    FRAME_ENVELOPE_RESERVE, MAX_HISTORY_FRAME_BYTES, MAX_HISTORY_MESSAGES, bounded_history,
};

fn message(content: impl Into<String>) -> ProjectedMessage {
    ProjectedMessage {
        role: "assistant".into(),
        content: content.into(),
        tool_calls: vec![],
        image_count: 0,
        skill_info: None,
    }
}

#[test]
fn retains_the_latest_bounded_suffix() {
    let history = bounded_history(
        "session".into(),
        (0..MAX_HISTORY_MESSAGES + 3)
            .map(|index| message(index.to_string()))
            .collect(),
    );
    assert_eq!(history.messages.len(), MAX_HISTORY_MESSAGES);
    assert_eq!(history.messages[0].content, "3");
    assert_eq!(history.messages.last().unwrap().content, "514");
    assert!(history.truncated);
}

#[test]
fn serialized_payload_stays_below_the_frame_budget() {
    let history = bounded_history(
        "session".into(),
        (0..64)
            .map(|index| message(format!("{index}:{}", "x".repeat(256 * 1024))))
            .collect(),
    );
    let encoded =
        serde_json::to_vec(&AgentEventPayload::SessionHistoryLoaded(history.clone())).unwrap();
    assert!(encoded.len() <= MAX_HISTORY_FRAME_BYTES - FRAME_ENVELOPE_RESERVE);
    assert!(history.truncated);
    assert!(history.messages.len() < 64);
}

#[test]
fn oversized_latest_message_fails_closed() {
    let history = bounded_history(
        "session".into(),
        vec![message("界".repeat(MAX_HISTORY_FRAME_BYTES))],
    );
    assert!(history.messages.is_empty());
    assert!(history.truncated);
}

#[test]
fn preserves_small_history_and_session_identity() {
    let history = bounded_history("session-1".into(), vec![message("persisted")]);
    assert_eq!(history.session_id, "session-1");
    assert_eq!(history.messages[0].content, "persisted");
    assert!(!history.truncated);
}
