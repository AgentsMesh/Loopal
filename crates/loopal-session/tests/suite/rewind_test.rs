use loopal_session::AgentConversation;
use loopal_session::SessionMessage;

fn user_msg(text: &str) -> SessionMessage {
    SessionMessage {
        role: "user".into(),
        content: text.into(),
        tool_calls: vec![],
        image_count: 0,
        skill_info: None,
    }
}

fn asst_msg(text: &str) -> SessionMessage {
    SessionMessage {
        role: "assistant".into(),
        content: text.into(),
        tool_calls: vec![],
        image_count: 0,
        skill_info: None,
    }
}

fn sys_msg(text: &str) -> SessionMessage {
    SessionMessage {
        role: "system".into(),
        content: text.into(),
        tool_calls: vec![],
        image_count: 0,
        skill_info: None,
    }
}

fn conv_with_messages(msgs: Vec<SessionMessage>) -> AgentConversation {
    let mut conv = AgentConversation::default();
    conv.turn_count = msgs.iter().filter(|m| m.role == "user").count() as u32;
    conv.messages = msgs;
    conv
}

#[test]
fn truncate_to_zero_clears_all() {
    let mut conv = conv_with_messages(vec![
        user_msg("q1"),
        asst_msg("a1"),
        user_msg("q2"),
        asst_msg("a2"),
    ]);
    loopal_session::rewind::truncate_display_to_turn(&mut conv, 0);
    assert!(conv.messages.is_empty());
    assert_eq!(conv.turn_count, 0);
}

#[test]
fn truncate_keeps_first_n_turns() {
    let mut conv = conv_with_messages(vec![
        user_msg("q1"),
        asst_msg("a1"),
        user_msg("q2"),
        asst_msg("a2"),
        user_msg("q3"),
        asst_msg("a3"),
    ]);
    loopal_session::rewind::truncate_display_to_turn(&mut conv, 2);
    assert_eq!(conv.messages.len(), 4); // q1, a1, q2, a2
    assert_eq!(conv.messages[0].content, "q1");
    assert_eq!(conv.messages[3].content, "a2");
    assert_eq!(conv.turn_count, 2);
}

#[test]
fn truncate_preserves_system_messages_within_turns() {
    let mut conv = conv_with_messages(vec![
        user_msg("q1"),
        asst_msg("a1"),
        sys_msg("model switched"),
        user_msg("q2"),
        asst_msg("a2"),
    ]);
    loopal_session::rewind::truncate_display_to_turn(&mut conv, 1);
    assert_eq!(conv.messages.len(), 3);
    assert_eq!(conv.messages[2].role, "system");
}

#[test]
fn truncate_remaining_exceeds_actual_keeps_all() {
    let mut conv = conv_with_messages(vec![user_msg("q1"), asst_msg("a1")]);
    loopal_session::rewind::truncate_display_to_turn(&mut conv, 10);
    assert_eq!(conv.messages.len(), 2);
    assert_eq!(conv.turn_count, 10);
}

#[test]
fn truncate_clears_streaming_text() {
    let mut conv = conv_with_messages(vec![user_msg("q1"), asst_msg("a1"), user_msg("q2")]);
    conv.streaming_text = "partial output...".into();
    loopal_session::rewind::truncate_display_to_turn(&mut conv, 1);
    assert!(conv.streaming_text.is_empty());
}
