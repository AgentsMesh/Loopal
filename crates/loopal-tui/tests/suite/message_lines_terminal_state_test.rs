use loopal_tui::views::progress::message_to_lines;
use loopal_view_state::SessionMessage;

use crate::message_lines_fixture::{
    all_text, cancelled_state, done_failure, done_success, pending_call, stale_state,
};

#[test]
fn done_success_renders_duration_label() {
    let mut tc = pending_call("Bash", "Bash(echo hi)");
    tc.state = done_success("hi");
    let m = SessionMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: vec![tc],
        ..Default::default()
    };
    let text = all_text(&message_to_lines(&m, 80));
    assert!(
        text.contains("Done in"),
        "success body should display Done in: {text}"
    );
}

#[test]
fn done_failure_renders_failure_label() {
    let mut tc = pending_call("Bash", "Bash(bad)");
    tc.state = done_failure("ENOENT");
    let m = SessionMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: vec![tc],
        ..Default::default()
    };
    let text = all_text(&message_to_lines(&m, 80));
    assert!(
        text.contains("Failed in"),
        "failure body should display Failed in: {text}"
    );
    assert!(text.contains("ENOENT"));
}

#[test]
fn stale_state_renders_with_reason() {
    let mut tc = pending_call("Bash", "Bash(slow)");
    tc.state = stale_state(loopal_view_state::StaleReason::TurnEnded);
    let m = SessionMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: vec![tc],
        ..Default::default()
    };
    let text = all_text(&message_to_lines(&m, 80));
    assert!(
        text.contains("Stale"),
        "stale body should contain Stale: {text}"
    );
    assert!(text.contains("turn ended"));
}

#[test]
fn cancelled_state_renders_with_cause() {
    let mut tc = pending_call("Bash", "Bash(hung)");
    tc.state = cancelled_state(loopal_view_state::CancelCause::UserInterrupt);
    let m = SessionMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: vec![tc],
        ..Default::default()
    };
    let text = all_text(&message_to_lines(&m, 80));
    assert!(text.contains("Cancelled"), "cancelled body: {text}");
    assert!(text.contains("user interrupt"));
}

#[test]
fn stale_duration_shown_in_human_format() {
    let mut tc = pending_call("Bash", "Bash(slow)");
    tc.state = stale_state(loopal_view_state::StaleReason::WatchdogTimeout);
    let m = SessionMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: vec![tc],
        ..Default::default()
    };
    let text = all_text(&message_to_lines(&m, 80));
    assert!(text.contains("5.0s") || text.contains("after"));
}
