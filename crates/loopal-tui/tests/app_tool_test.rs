use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::command::builtin_entries;
use loopal_protocol::ControlCommand;
use loopal_protocol::{AgentEvent, AgentEventPayload, UserQuestionResponse};
use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "test-model".to_string(),
        "act".to_string(),
        control_tx,
        perm_tx,
        question_tx,
    );
    App::new(session, builtin_entries(), std::env::temp_dir())
}

#[test]
fn test_handle_tool_permission_request() {
    let app = make_app();
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
            id: "tool-1".to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({"command": "ls"}),
        }));

    let state = app.session.lock();
    let perm = state.pending_permission.as_ref().expect("should have pending permission");
    assert_eq!(perm.id, "tool-1");
    assert_eq!(perm.name, "bash");
}

#[test]
fn test_handle_tool_permission_flushes_streaming() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "about to call tool".to_string(),
    }));
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
            id: "perm-1".to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({"command": "rm -rf /"}),
        }));

    let state = app.session.lock();
    assert!(state.streaming_text.is_empty());
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].content, "about to call tool");
    assert!(state.pending_permission.is_some());
}

#[test]
fn test_handle_tool_call_event() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "ls"}),
    }));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].role, "assistant");
    assert_eq!(state.messages[0].tool_calls.len(), 1);
    assert_eq!(state.messages[0].tool_calls[0].name, "bash");
    assert_eq!(state.messages[0].tool_calls[0].status, "pending");
}

#[test]
fn test_handle_tool_result_updates_status() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "ls"}),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        result: "file1.txt\nfile2.txt".to_string(),
        is_error: false,
    }));

    assert_eq!(app.session.lock().messages[0].tool_calls[0].status, "success");
}

#[test]
fn test_handle_tool_result_error_status() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-err".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "fail"}),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc-err".to_string(),
        name: "bash".to_string(),
        result: "command failed".to_string(),
        is_error: true,
    }));

    assert_eq!(app.session.lock().messages[0].tool_calls[0].status, "error");
}

#[test]
fn test_handle_tool_call_appends_to_existing_assistant_message() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "Let me run that.".to_string(),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({"command": "ls"}),
    }));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].role, "assistant");
    assert_eq!(state.messages[0].content, "Let me run that.");
    assert_eq!(state.messages[0].tool_calls.len(), 1);
}

#[test]
fn test_handle_tool_call_second_tool_on_same_message() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({}),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-2".to_string(),
        name: "Read".to_string(),
        input: serde_json::json!({}),
    }));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].tool_calls.len(), 2);
    assert_eq!(state.messages[0].tool_calls[0].name, "bash");
    assert_eq!(state.messages[0].tool_calls[1].name, "Read");
}
