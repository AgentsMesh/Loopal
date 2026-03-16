use loopagent_tui::app::{App, AppState};
use loopagent_types::event::AgentEvent;
use tokio::sync::mpsc;

fn make_app() -> App {
    let (tx, _rx) = mpsc::channel::<AgentEvent>(16);
    App::new("test-model".to_string(), "act".to_string(), tx)
}

#[test]
fn test_handle_tool_permission_request() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::ToolPermissionRequest {
        id: "tool-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "ls"}),
    });

    match &app.state {
        AppState::ToolConfirm { id, name, .. } => {
            assert_eq!(id, "tool-1");
            assert_eq!(name, "bash");
        }
        other => panic!("expected ToolConfirm, got {:?}", other),
    }
}

#[test]
fn test_handle_tool_permission_flushes_streaming() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::Stream {
        text: "about to call tool".to_string(),
    });
    app.handle_agent_event(AgentEvent::ToolPermissionRequest {
        id: "perm-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "rm -rf /"}),
    });

    assert!(app.streaming_text.is_empty(), "streaming text should be flushed");
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].content, "about to call tool");
    assert!(matches!(app.state, AppState::ToolConfirm { .. }));
}

#[test]
fn test_handle_tool_call_event() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "ls"}),
    });

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, "assistant");
    assert_eq!(app.messages[0].tool_calls.len(), 1);
    assert_eq!(app.messages[0].tool_calls[0].name, "bash");
    assert_eq!(app.messages[0].tool_calls[0].status, "pending");
}

#[test]
fn test_handle_tool_result_updates_status() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "ls"}),
    });
    app.handle_agent_event(AgentEvent::ToolResult {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        result: "file1.txt\nfile2.txt".to_string(),
        is_error: false,
    });

    assert_eq!(app.messages[0].tool_calls[0].status, "success");
}

#[test]
fn test_handle_tool_result_error_status() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-err".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "fail"}),
    });
    app.handle_agent_event(AgentEvent::ToolResult {
        id: "tc-err".to_string(),
        name: "bash".to_string(),
        result: "command failed".to_string(),
        is_error: true,
    });

    assert_eq!(app.messages[0].tool_calls[0].status, "error");
}

#[test]
fn test_handle_tool_call_appends_to_existing_assistant_message() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::Stream {
        text: "Let me run that.".to_string(),
    });
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "ls"}),
    });

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, "assistant");
    assert_eq!(app.messages[0].content, "Let me run that.");
    assert_eq!(app.messages[0].tool_calls.len(), 1);
}

#[test]
fn test_handle_tool_call_second_tool_on_same_message() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({}),
    });
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-2".to_string(),
        name: "Read".to_string(),
        input: serde_json::json!({}),
    });

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].tool_calls.len(), 2);
    assert_eq!(app.messages[0].tool_calls[0].name, "bash");
    assert_eq!(app.messages[0].tool_calls[1].name, "Read");
}

