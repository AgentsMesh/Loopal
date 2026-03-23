use loopal_protocol::ControlCommand;
use loopal_protocol::{AgentEvent, AgentEventPayload, ImageAttachment, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::command::builtin_entries;
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
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
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
    assert_eq!(result.map(|c| c.text), Some("hello world".to_string()));
    assert!(app.input.is_empty());
    assert_eq!(app.input_cursor, 0);
}

#[test]
fn test_awaiting_input_sets_idle() {
    let (app, _, _) = make_app();
    assert!(!app.session.lock().agent_idle);
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));
    assert!(app.session.lock().agent_idle);
}

#[test]
fn test_awaiting_input_forwards_inbox_message() {
    let (app, _, _) = make_app();
    {
        let mut state = app.session.lock();
        state.inbox.push("queued".into());
    }
    // AwaitingInput sets idle=true then tries forward
    let forwarded = app
        .session
        .handle_event(AgentEvent::root(AgentEventPayload::AwaitingInput));
    assert_eq!(forwarded.map(|c| c.text), Some("queued".to_string()));
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
        state.inbox.push("first".into());
        state.inbox.push("second".into());
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

fn sample_image(label: &str) -> ImageAttachment {
    ImageAttachment {
        media_type: "image/png".to_string(),
        data: format!("base64-{label}"),
    }
}

#[test]
fn test_submit_input_with_images() {
    let (mut app, _, _) = make_app();
    app.input = "describe this".to_string();
    app.pending_images = vec![sample_image("a"), sample_image("b")];

    let result = app.submit_input().expect("should return content");
    assert_eq!(result.text, "describe this");
    assert_eq!(result.images.len(), 2);
    assert_eq!(result.images[0].data, "base64-a");
    assert_eq!(result.images[1].data, "base64-b");
}

#[test]
fn test_submit_input_clears_pending_images() {
    let (mut app, _, _) = make_app();
    app.input = "check".to_string();
    app.pending_images = vec![sample_image("x")];

    let _ = app.submit_input();
    assert!(app.pending_images.is_empty());
    assert!(app.input.is_empty());
    assert_eq!(app.input_cursor, 0);
}

#[test]
fn test_submit_input_images_only() {
    let (mut app, _, _) = make_app();
    app.input = String::new(); // empty text
    app.pending_images = vec![sample_image("only")];

    let result = app.submit_input();
    assert!(result.is_some(), "images-only input should not be None");
    let content = result.unwrap();
    assert!(content.text.is_empty());
    assert_eq!(content.images.len(), 1);
}
