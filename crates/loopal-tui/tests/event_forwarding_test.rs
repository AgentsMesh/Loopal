//! Tests for forwarding specific AgentEvent variants through EventHandler.

use loopal_tui::event::{AppEvent, EventHandler};
use loopal_types::event::{AgentEvent, AgentEventPayload};
use tokio::sync::mpsc;

/// Helper: send an event and wait for a matching AppEvent::Agent variant.
async fn send_and_recv(event: AgentEvent) -> AgentEvent {
    let (agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(16);
    let mut handler = EventHandler::new(agent_rx);

    agent_tx.send(event).await.unwrap();

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(500), handler.next()).await {
            Ok(Some(AppEvent::Agent(e))) => return e,
            Ok(Some(_)) => continue,
            _ => break,
        }
    }
    panic!("did not receive expected agent event");
}

#[tokio::test]
async fn test_agent_error_event_forwarded() {
    let event = send_and_recv(AgentEvent::root(AgentEventPayload::Error {
        message: "test error".to_string(),
    }))
    .await;

    match event.payload {
        AgentEventPayload::Error { message } => assert_eq!(message, "test error"),
        other => panic!("expected Error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_agent_tool_call_event_forwarded() {
    let event = send_and_recv(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "ls"}),
    }))
    .await;

    match event.payload {
        AgentEventPayload::ToolCall { name, .. } => assert_eq!(name, "bash"),
        other => panic!("expected ToolCall, got {:?}", other),
    }
}

#[tokio::test]
async fn test_agent_token_usage_forwarded() {
    let event = send_and_recv(AgentEvent::root(AgentEventPayload::TokenUsage {
        input_tokens: 500,
        output_tokens: 200,
        context_window: 200_000,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
    }))
    .await;

    match event.payload {
        AgentEventPayload::TokenUsage {
            input_tokens,
            output_tokens,
            context_window, ..
        } => {
            assert_eq!(input_tokens, 500);
            assert_eq!(output_tokens, 200);
            assert_eq!(context_window, 200_000);
        }
        other => panic!("expected TokenUsage, got {:?}", other),
    }
}

#[tokio::test]
async fn test_agent_mode_changed_forwarded() {
    let event = send_and_recv(AgentEvent::root(AgentEventPayload::ModeChanged {
        mode: "plan".to_string(),
    }))
    .await;

    match event.payload {
        AgentEventPayload::ModeChanged { mode } => assert_eq!(mode, "plan"),
        other => panic!("expected ModeChanged, got {:?}", other),
    }
}
