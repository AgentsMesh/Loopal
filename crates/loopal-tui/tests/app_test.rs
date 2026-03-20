use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::command::builtin_entries;
use loopal_protocol::ControlCommand;
use loopal_protocol::{AgentEvent, AgentEventPayload, UserQuestionResponse};
use tokio::sync::mpsc;

fn make_app() -> (App, mpsc::Receiver<ControlCommand>, mpsc::Receiver<bool>) {
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, perm_rx) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "test-model".to_string(),
        "act".to_string(),
        control_tx,
        perm_tx,
        question_tx,
    );
    let app = App::new(session, builtin_entries(), std::env::temp_dir());
    (app, control_rx, perm_rx)
}

#[test]
fn test_app_new_initializes_correctly() {
    let (app, _, _) = make_app();
    assert!(!app.exiting);
    assert!(app.input.is_empty());
    assert_eq!(app.input_cursor, 0);
    assert_eq!(app.scroll_offset, 0);
    let state = app.session.lock();
    assert!(state.messages.is_empty());
    assert_eq!(state.model, "test-model");
    assert_eq!(state.mode, "act");
    assert_eq!(state.token_count(), 0);
    assert_eq!(state.context_window, 0);
    assert_eq!(state.turn_count, 0);
    assert!(state.streaming_text.is_empty());
    drop(state);
    assert!(app.input_history.is_empty());
    assert!(app.history_index.is_none());
}

#[test]
fn test_submit_input_empty_returns_none() {
    let (mut app, _, _) = make_app();
    app.input = "   ".to_string();
    assert!(app.submit_input().is_none());
}

#[test]
fn test_submit_input_returns_text_and_resets() {
    let (mut app, _, _) = make_app();
    app.input = "hello world".to_string();
    app.input_cursor = 11;

    let result = app.submit_input();
    assert_eq!(result, Some("hello world".to_string()));
    assert!(app.input.is_empty());
    assert_eq!(app.input_cursor, 0);
}

#[test]
fn test_awaiting_input_sets_idle() {
    let (app, _, _) = make_app();
    assert!(!app.session.lock().agent_idle);
    app.session.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));
    assert!(app.session.lock().agent_idle);
}

#[test]
fn test_awaiting_input_forwards_inbox_message() {
    let (app, _, _) = make_app();
    {
        let mut state = app.session.lock();
        state.inbox.push("queued".to_string());
    }
    // AwaitingInput sets idle=true then tries forward
    let forwarded = app.session.handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));
    assert_eq!(forwarded, Some("queued".to_string()));
    let state = app.session.lock();
    assert!(!state.agent_idle); // forwarding clears idle
    assert!(state.inbox.is_empty());
    assert_eq!(state.messages.last().unwrap().role, "user");
    assert_eq!(state.messages.last().unwrap().content, "queued");
}

#[test]
fn test_pop_inbox_to_input() {
    let (mut app, _, _) = make_app();
    {
        let mut state = app.session.lock();
        state.inbox.push("first".to_string());
        state.inbox.push("second".to_string());
    }
    assert!(app.pop_inbox_to_input());
    assert_eq!(app.input, "second");
    assert_eq!(app.input_cursor, 6);
    assert_eq!(app.session.lock().inbox.len(), 1);
}

#[test]
fn test_pop_inbox_empty_returns_false() {
    let (mut app, _, _) = make_app();
    assert!(!app.pop_inbox_to_input());
}
