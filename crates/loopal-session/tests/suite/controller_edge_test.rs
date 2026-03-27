//! Edge case tests for SessionController: token usage, mode, errors, inbox.

use loopal_protocol::{AgentEvent, AgentEventPayload};

use super::controller_test::make_controller;

#[test]
fn test_token_usage() {
    let (ctrl, _, _) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        context_window: 200_000,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        thinking_tokens: 0,
    }));

    let state = ctrl.lock();
    assert_eq!(state.input_tokens, 100);
    assert_eq!(state.output_tokens, 50);
    assert_eq!(state.context_window, 200_000);
    assert_eq!(state.token_count(), 150);
}

#[test]
fn test_mode_changed() {
    let (ctrl, _, _) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::ModeChanged {
        mode: "plan".to_string(),
    }));
    assert_eq!(ctrl.lock().mode, "plan");
}

#[test]
fn test_error_event() {
    let (ctrl, _, _) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::Error {
        message: "bad".to_string(),
    }));

    let state = ctrl.lock();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].role, "error");
}

#[test]
fn test_push_system_message() {
    let (ctrl, _, _) = make_controller();
    ctrl.push_system_message("hello".to_string());

    let state = ctrl.lock();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].role, "system");
    assert_eq!(state.messages[0].content, "hello");
}

#[test]
fn test_pop_inbox_to_edit() {
    let (ctrl, _, _) = make_controller();
    ctrl.lock().inbox.push("first".into());
    ctrl.lock().inbox.push("second".into());

    assert_eq!(
        ctrl.pop_inbox_to_edit().map(|c| c.text),
        Some("second".to_string())
    );
    assert_eq!(ctrl.lock().inbox.len(), 1);
    assert_eq!(
        ctrl.pop_inbox_to_edit().map(|c| c.text),
        Some("first".to_string())
    );
    assert!(ctrl.pop_inbox_to_edit().is_none());
}
