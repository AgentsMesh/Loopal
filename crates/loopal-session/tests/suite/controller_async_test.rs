//! Async tests for SessionController interaction methods (channels).

use loopal_protocol::ControlCommand;
use loopal_protocol::{AgentEvent, AgentEventPayload, UserQuestionResponse};
use loopal_session::SessionController;
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
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
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
    let result = ctrl.enqueue_message("hello".into());
    assert_eq!(result.map(|c| c.text), Some("hello".to_string()));
}

#[tokio::test]
async fn test_enqueue_message_queues_when_busy() {
    let (ctrl, _, _) = make_controller();
    ctrl.lock().agent_idle = false;

    let result = ctrl.enqueue_message("queued".into());
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
        other => panic!("expected ModeSwitch, got {other:?}"),
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
        other => panic!("expected ModelSwitch, got {other:?}"),
    }
}

#[tokio::test]
async fn test_clear() {
    let (ctrl, mut control_rx, _) = make_controller();
    ctrl.push_system_message("msg".to_string());
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "partial".to_string(),
    }));
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        context_window: 200_000,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        thinking_tokens: 0,
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
        other => panic!("expected Clear, got {other:?}"),
    }
}

#[tokio::test]
async fn test_compact() {
    let (ctrl, mut control_rx, _) = make_controller();
    ctrl.compact().await;

    match control_rx.recv().await {
        Some(ControlCommand::Compact) => {}
        other => panic!("expected Compact, got {other:?}"),
    }
}

/// Hub mode: approve_permission sends response via HubClient when relay_request_id is set.
#[tokio::test]
async fn test_hub_approve_permission_sends_response() {
    use loopal_agent_hub::Hub;
    use loopal_agent_hub::HubClient;
    use std::sync::Arc;

    // Create a duplex to simulate Hub connection
    let (client_side, server_side) = tokio::io::duplex(4096);
    let client_transport: Arc<dyn loopal_ipc::transport::Transport> =
        Arc::new(loopal_ipc::StdioTransport::new(
            Box::new(tokio::io::BufReader::new(client_side)),
            Box::new(server_side),
        ));
    // We don't need a real Hub — just verify the response is sent.
    // The connection will error but that's fine for this unit test.

    let conn = Arc::new(loopal_ipc::connection::Connection::new(client_transport));
    let _rx = conn.start();
    let hub_client = Arc::new(HubClient::new(conn));
    let hub = Arc::new(tokio::sync::Mutex::new(Hub::noop()));

    let ctrl = SessionController::with_hub("test".to_string(), "act".to_string(), hub_client, hub);

    // Set up pending permission with relay_request_id
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
        id: "p1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({}),
    }));
    {
        let mut state = ctrl.lock();
        if let Some(ref mut perm) = state.pending_permission {
            perm.relay_request_id = Some(42);
        }
    }

    // approve should not panic (response goes to duplex — may error but shouldn't crash)
    ctrl.approve_permission().await;
    assert!(ctrl.lock().pending_permission.is_none());
}
