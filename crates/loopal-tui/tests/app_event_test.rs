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
fn test_handle_stream_event_buffers_text() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "Hello ".to_string(),
    }));
    assert_eq!(app.session.lock().streaming_text, "Hello ");

    app.session.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "world".to_string(),
    }));
    assert_eq!(app.session.lock().streaming_text, "Hello world");
}

#[test]
fn test_handle_awaiting_input_flushes_and_increments_turn() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "response text".to_string(),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));

    let state = app.session.lock();
    assert!(state.streaming_text.is_empty());
    assert_eq!(state.turn_count, 1);
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].role, "assistant");
    assert_eq!(state.messages[0].content, "response text");
}

#[test]
fn test_handle_error_event() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::Error {
        message: "something went wrong".to_string(),
    }));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].role, "error");
    assert_eq!(state.messages[0].content, "something went wrong");
}

#[test]
fn test_handle_token_usage() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        context_window: 200_000,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
    }));

    let state = app.session.lock();
    assert_eq!(state.token_count(), 150);
    assert_eq!(state.context_window, 200_000);
}

#[test]
fn test_handle_mode_changed() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ModeChanged {
        mode: "plan".to_string(),
    }));
    assert_eq!(app.session.lock().mode, "plan");
}

#[test]
fn test_handle_max_turns_reached() {
    let app = make_app();
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::MaxTurnsReached { turns: 50 }));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].role, "system");
    assert!(state.messages[0].content.contains("50"));
}

#[test]
fn test_handle_started_event() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::Started));
    let state = app.session.lock();
    assert!(state.messages.is_empty());
    assert!(state.streaming_text.is_empty());
}

#[test]
fn test_handle_finished_event_flushes_streaming() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "final text".to_string(),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::Finished));

    let state = app.session.lock();
    assert!(state.streaming_text.is_empty());
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].content, "final text");
}

#[test]
fn test_flush_streaming_empty_is_noop() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));
    let state = app.session.lock();
    assert!(state.messages.is_empty());
    assert_eq!(state.turn_count, 1);
}
