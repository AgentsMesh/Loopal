use loopagent_tui::app::App;
use loopagent_tui::command::builtin_entries;
use loopagent_types::event::AgentEvent;
use tokio::sync::mpsc;

fn make_app() -> App {
    let (tx, _rx) = mpsc::channel::<AgentEvent>(16);
    App::new(
        "test-model".to_string(),
        "act".to_string(),
        tx,
        builtin_entries(),
        std::env::temp_dir(),
    )
}

#[test]
fn test_handle_stream_event_buffers_text() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::Stream {
        text: "Hello ".to_string(),
    });
    assert_eq!(app.streaming_text, "Hello ");

    app.handle_agent_event(AgentEvent::Stream {
        text: "world".to_string(),
    });
    assert_eq!(app.streaming_text, "Hello world");
}

#[test]
fn test_handle_awaiting_input_flushes_and_increments_turn() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::Stream {
        text: "response text".to_string(),
    });
    app.handle_agent_event(AgentEvent::AwaitingInput);

    assert!(app.streaming_text.is_empty());
    assert_eq!(app.turn_count, 1);
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, "assistant");
    assert_eq!(app.messages[0].content, "response text");
}

#[test]
fn test_handle_error_event() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::Error {
        message: "something went wrong".to_string(),
    });

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, "error");
    assert_eq!(app.messages[0].content, "something went wrong");
}

#[test]
fn test_handle_token_usage() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        context_window: 200_000,
    });

    assert_eq!(app.token_count, 150);
    assert_eq!(app.context_window, 200_000);
}

#[test]
fn test_handle_mode_changed() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::ModeChanged {
        mode: "plan".to_string(),
    });
    assert_eq!(app.mode, "plan");
}

#[test]
fn test_handle_max_turns_reached() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::MaxTurnsReached { turns: 50 });

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, "system");
    assert!(app.messages[0].content.contains("50"));
}

#[test]
fn test_handle_started_event() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::Started);
    assert!(app.messages.is_empty());
    assert!(app.streaming_text.is_empty());
}

#[test]
fn test_handle_finished_event_flushes_streaming() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::Stream {
        text: "final text".to_string(),
    });
    app.handle_agent_event(AgentEvent::Finished);

    assert!(app.streaming_text.is_empty(), "streaming_text should be flushed");
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].content, "final text");
}

#[test]
fn test_flush_streaming_appends_to_existing_assistant_message() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::Stream {
        text: "first chunk".to_string(),
    });
    app.handle_agent_event(AgentEvent::AwaitingInput);

    app.handle_agent_event(AgentEvent::Stream {
        text: " second chunk".to_string(),
    });
    app.handle_agent_event(AgentEvent::AwaitingInput);

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].content, "first chunk second chunk");
}

#[test]
fn test_flush_streaming_creates_new_message_after_tool_call() {
    let mut app = make_app();

    app.handle_agent_event(AgentEvent::Stream {
        text: "before tool".to_string(),
    });
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({}),
    });

    app.handle_agent_event(AgentEvent::Stream {
        text: "after tool".to_string(),
    });
    app.handle_agent_event(AgentEvent::AwaitingInput);

    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].content, "before tool");
    assert!(!app.messages[0].tool_calls.is_empty());
    assert_eq!(app.messages[1].content, "after tool");
    assert!(app.messages[1].tool_calls.is_empty());
}

#[test]
fn test_flush_streaming_empty_is_noop() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::AwaitingInput);
    assert!(app.messages.is_empty());
    assert_eq!(app.turn_count, 1);
}

#[test]
fn test_flush_streaming_new_message_when_last_is_not_assistant() {
    let mut app = make_app();
    app.messages.push(loopagent_tui::app::DisplayMessage {
        role: "user".to_string(),
        content: "hi".to_string(),
        tool_calls: Vec::new(),
    });
    app.streaming_text = "response".to_string();
    app.handle_agent_event(AgentEvent::AwaitingInput);

    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[1].role, "assistant");
    assert_eq!(app.messages[1].content, "response");
}

#[test]
fn test_flush_streaming_new_message_when_assistant_has_tool_calls() {
    let mut app = make_app();
    app.messages.push(loopagent_tui::app::DisplayMessage {
        role: "assistant".to_string(),
        content: "let me do that".to_string(),
        tool_calls: vec![loopagent_tui::app::DisplayToolCall {
            name: "bash".to_string(),
            status: "success".to_string(),
            summary: "done".to_string(),
        }],
    });
    app.streaming_text = "new response".to_string();
    app.handle_agent_event(AgentEvent::AwaitingInput);

    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[1].role, "assistant");
    assert_eq!(app.messages[1].content, "new response");
}
