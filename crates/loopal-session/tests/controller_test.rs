//! Tests for SessionController event handling and state management.

use loopal_session::SessionController;
use loopal_protocol::ControlCommand;
use loopal_protocol::{AgentEvent, AgentEventPayload, UserQuestionResponse};
use tokio::sync::mpsc;

fn make_controller() -> (SessionController, mpsc::Receiver<ControlCommand>, mpsc::Receiver<bool>) {
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, perm_rx) = mpsc::channel::<bool>(16);
    let (question_tx, _question_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let ctrl = SessionController::new(
        "test-model".to_string(),
        "act".to_string(),
        control_tx,
        perm_tx,
        question_tx,
    );
    (ctrl, control_rx, perm_rx)
}

#[test]
fn test_initial_state() {
    let (ctrl, _, _) = make_controller();
    let state = ctrl.lock();
    assert_eq!(state.model, "test-model");
    assert_eq!(state.mode, "act");
    assert!(state.messages.is_empty());
    assert!(state.streaming_text.is_empty());
    assert!(!state.agent_idle);
    assert_eq!(state.turn_count, 0);
    assert_eq!(state.token_count(), 0);
    assert!(state.pending_permission.is_none());
    assert!(state.inbox.is_empty());
}

#[test]
fn test_stream_event() {
    let (ctrl, _, _) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::Stream { text: "hello".to_string() }));
    assert_eq!(ctrl.lock().streaming_text, "hello");

    ctrl.handle_event(AgentEvent::root(AgentEventPayload::Stream { text: " world".to_string() }));
    assert_eq!(ctrl.lock().streaming_text, "hello world");
}

#[test]
fn test_awaiting_input_flushes_streaming() {
    let (ctrl, _, _) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::Stream { text: "response".to_string() }));
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));

    let state = ctrl.lock();
    assert!(state.streaming_text.is_empty());
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].role, "assistant");
    assert_eq!(state.messages[0].content, "response");
    assert_eq!(state.turn_count, 1);
    assert!(state.agent_idle);
}

#[test]
fn test_awaiting_input_forwards_inbox() {
    let (ctrl, _, _) = make_controller();
    ctrl.lock().inbox.push("queued msg".to_string());

    let forwarded = ctrl.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));
    assert_eq!(forwarded, Some("queued msg".to_string()));

    let state = ctrl.lock();
    assert!(!state.agent_idle); // forwarding clears idle
    assert!(state.inbox.is_empty());
    assert_eq!(state.messages.last().unwrap().role, "user");
    assert_eq!(state.messages.last().unwrap().content, "queued msg");
}

#[test]
fn test_awaiting_input_no_inbox_stays_idle() {
    let (ctrl, _, _) = make_controller();
    let forwarded = ctrl.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));
    assert!(forwarded.is_none());
    assert!(ctrl.lock().agent_idle);
}

#[test]
fn test_tool_call_and_result() {
    let (ctrl, _, _) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "ls"}),
    }));
    assert_eq!(ctrl.lock().messages[0].tool_calls[0].status, "pending");

    ctrl.handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        result: "file.txt".to_string(),
        is_error: false,
    }));
    assert_eq!(ctrl.lock().messages[0].tool_calls[0].status, "success");
}

#[test]
fn test_permission_request() {
    let (ctrl, _, _) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
        id: "p1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({}),
    }));

    let state = ctrl.lock();
    assert!(state.pending_permission.is_some());
    assert_eq!(state.pending_permission.as_ref().unwrap().name, "bash");
}

#[test]
fn test_token_usage() {
    let (ctrl, _, _) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        context_window: 200_000,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
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
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::ModeChanged { mode: "plan".to_string() }));
    assert_eq!(ctrl.lock().mode, "plan");
}

#[test]
fn test_error_event() {
    let (ctrl, _, _) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::Error { message: "bad".to_string() }));

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
    ctrl.lock().inbox.push("first".to_string());
    ctrl.lock().inbox.push("second".to_string());

    assert_eq!(ctrl.pop_inbox_to_edit(), Some("second".to_string()));
    assert_eq!(ctrl.lock().inbox.len(), 1);
    assert_eq!(ctrl.pop_inbox_to_edit(), Some("first".to_string()));
    assert!(ctrl.pop_inbox_to_edit().is_none());
}
