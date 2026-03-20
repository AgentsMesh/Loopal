//! Async tests for SessionController interaction methods (channels).

use loopal_session::SessionController;
use loopal_protocol::ControlCommand;
use loopal_protocol::{AgentEvent, AgentEventPayload, UserQuestionResponse};
use tokio::sync::mpsc;

fn make_controller() -> (
    SessionController,
    mpsc::Receiver<ControlCommand>,
    mpsc::Receiver<bool>,
) {
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

#[tokio::test]
async fn test_approve_permission() {
    let (ctrl, _, mut perm_rx) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
        id: "p1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({}),
    }));

    ctrl.approve_permission().await;
    assert!(ctrl.lock().pending_permission.is_none());
    assert_eq!(perm_rx.recv().await, Some(true));
}

#[tokio::test]
async fn test_deny_permission() {
    let (ctrl, _, mut perm_rx) = make_controller();
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
        id: "p1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({}),
    }));

    ctrl.deny_permission().await;
    assert!(ctrl.lock().pending_permission.is_none());
    assert_eq!(perm_rx.recv().await, Some(false));
}

#[tokio::test]
async fn test_enqueue_message_forwards_when_idle() {
    let (ctrl, _, _) = make_controller();
    ctrl.lock().agent_idle = true;
    let result = ctrl.enqueue_message("hello".to_string());
    assert_eq!(result, Some("hello".to_string()));
}

#[tokio::test]
async fn test_enqueue_message_queues_when_busy() {
    let (ctrl, _, _) = make_controller();
    ctrl.lock().agent_idle = false;

    let result = ctrl.enqueue_message("queued".to_string());
    assert!(result.is_none());
    assert_eq!(ctrl.lock().inbox.len(), 1);
}

#[tokio::test]
async fn test_switch_mode() {
    let (ctrl, mut control_rx, _) = make_controller();
    ctrl.switch_mode(loopal_protocol::AgentMode::Plan).await;

    assert_eq!(ctrl.lock().mode, "plan");
    match control_rx.recv().await {
        Some(ControlCommand::ModeSwitch(m)) => {
            assert!(matches!(m, loopal_protocol::AgentMode::Plan));
        }
        other => panic!("expected ModeSwitch, got {:?}", other),
    }
}

#[tokio::test]
async fn test_switch_model() {
    let (ctrl, mut control_rx, _) = make_controller();
    ctrl.switch_model("gpt-4".to_string()).await;

    {
        let state = ctrl.lock();
        assert_eq!(state.model, "gpt-4");
        assert_eq!(state.messages.len(), 1);
        assert!(state.messages[0].content.contains("gpt-4"));
    }

    match control_rx.recv().await {
        Some(ControlCommand::ModelSwitch(m)) => assert_eq!(m, "gpt-4"),
        other => panic!("expected ModelSwitch, got {:?}", other),
    }
}

#[tokio::test]
async fn test_clear() {
    let (ctrl, mut control_rx, _) = make_controller();
    ctrl.push_system_message("msg".to_string());
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::Stream { text: "partial".to_string() }));
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        context_window: 200_000,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
    }));

    ctrl.clear().await;

    {
        let state = ctrl.lock();
        assert!(state.messages.is_empty());
        assert!(state.inbox.is_empty());
        assert!(state.streaming_text.is_empty());
        assert_eq!(state.turn_count, 0);
        assert_eq!(state.input_tokens, 0);
        assert_eq!(state.output_tokens, 0);
    }

    match control_rx.recv().await {
        Some(ControlCommand::Clear) => {}
        other => panic!("expected Clear, got {:?}", other),
    }
}

#[tokio::test]
async fn test_compact() {
    let (ctrl, mut control_rx, _) = make_controller();
    ctrl.compact().await;

    match control_rx.recv().await {
        Some(ControlCommand::Compact) => {}
        other => panic!("expected Compact, got {:?}", other),
    }
}
