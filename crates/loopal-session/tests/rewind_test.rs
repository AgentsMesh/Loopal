use loopal_session::DisplayMessage;
use loopal_session::state::SessionState;

fn user_msg(text: &str) -> DisplayMessage {
    DisplayMessage { role: "user".into(), content: text.into(), tool_calls: vec![] }
}

fn asst_msg(text: &str) -> DisplayMessage {
    DisplayMessage { role: "assistant".into(), content: text.into(), tool_calls: vec![] }
}

fn sys_msg(text: &str) -> DisplayMessage {
    DisplayMessage { role: "system".into(), content: text.into(), tool_calls: vec![] }
}

fn state_with_messages(msgs: Vec<DisplayMessage>) -> SessionState {
    let mut s = SessionState::new("test-model".into(), "act".into());
    s.messages = msgs;
    s.turn_count = s.messages.iter().filter(|m| m.role == "user").count() as u32;
    s
}

#[test]
fn truncate_to_zero_clears_all() {
    let mut state = state_with_messages(vec![
        user_msg("q1"), asst_msg("a1"), user_msg("q2"), asst_msg("a2"),
    ]);
    loopal_session::rewind::truncate_display_to_turn(&mut state, 0);
    assert!(state.messages.is_empty());
    assert_eq!(state.turn_count, 0);
}

#[test]
fn truncate_keeps_first_n_turns() {
    let mut state = state_with_messages(vec![
        user_msg("q1"), asst_msg("a1"),
        user_msg("q2"), asst_msg("a2"),
        user_msg("q3"), asst_msg("a3"),
    ]);
    loopal_session::rewind::truncate_display_to_turn(&mut state, 2);
    assert_eq!(state.messages.len(), 4); // q1, a1, q2, a2
    assert_eq!(state.messages[0].content, "q1");
    assert_eq!(state.messages[3].content, "a2");
    assert_eq!(state.turn_count, 2);
}

#[test]
fn truncate_preserves_system_messages_within_turns() {
    let mut state = state_with_messages(vec![
        user_msg("q1"), asst_msg("a1"),
        sys_msg("model switched"),
        user_msg("q2"), asst_msg("a2"),
    ]);
    loopal_session::rewind::truncate_display_to_turn(&mut state, 1);
    // Keeps: q1, a1, system msg (all before user turn 2)
    assert_eq!(state.messages.len(), 3);
    assert_eq!(state.messages[2].role, "system");
}

#[test]
fn truncate_remaining_exceeds_actual_keeps_all() {
    let mut state = state_with_messages(vec![
        user_msg("q1"), asst_msg("a1"),
    ]);
    loopal_session::rewind::truncate_display_to_turn(&mut state, 10);
    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.turn_count, 10); // set to remaining_turns parameter
}

#[test]
fn truncate_clears_streaming_text() {
    let mut state = state_with_messages(vec![
        user_msg("q1"), asst_msg("a1"),
        user_msg("q2"),
    ]);
    state.streaming_text = "partial output...".into();
    loopal_session::rewind::truncate_display_to_turn(&mut state, 1);
    assert!(state.streaming_text.is_empty());
}
