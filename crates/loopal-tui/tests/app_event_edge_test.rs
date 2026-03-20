//! Edge cases for flush_streaming behavior.

use loopal_session::{DisplayMessage, DisplayToolCall, SessionController};
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
fn test_flush_streaming_appends_to_existing_assistant_message() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "first chunk".to_string(),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));

    app.session.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: " second chunk".to_string(),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].content, "first chunk second chunk");
}

#[test]
fn test_flush_streaming_creates_new_message_after_tool_call() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "before tool".to_string(),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({}),
    }));

    app.session.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "after tool".to_string(),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.messages[0].content, "before tool");
    assert!(!state.messages[0].tool_calls.is_empty());
    assert_eq!(state.messages[1].content, "after tool");
    assert!(state.messages[1].tool_calls.is_empty());
}

#[test]
fn test_flush_streaming_new_message_when_last_is_not_assistant() {
    let app = make_app();
    {
        let mut state = app.session.lock();
        state.messages.push(DisplayMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
            tool_calls: Vec::new(),
        });
        state.streaming_text = "response".to_string();
    }
    app.session.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.messages[1].role, "assistant");
    assert_eq!(state.messages[1].content, "response");
}

#[test]
fn test_flush_streaming_new_message_when_assistant_has_tool_calls() {
    let app = make_app();
    {
        let mut state = app.session.lock();
        state.messages.push(DisplayMessage {
            role: "assistant".to_string(),
            content: "let me do that".to_string(),
            tool_calls: vec![DisplayToolCall {
                name: "bash".to_string(),
                status: "success".to_string(),
                summary: "done".to_string(),
                result: Some("done".to_string()),
            }],
        });
        state.streaming_text = "new response".to_string();
    }
    app.session.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.messages[1].role, "assistant");
    assert_eq!(state.messages[1].content, "new response");
}
