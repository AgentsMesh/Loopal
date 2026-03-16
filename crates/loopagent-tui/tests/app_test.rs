use loopagent_tui::app::{App, AppState};
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
fn test_app_new_initializes_correctly() {
    let app = make_app();
    assert_eq!(app.state, AppState::Running);
    assert!(app.messages.is_empty());
    assert!(app.input.is_empty());
    assert_eq!(app.input_cursor, 0);
    assert_eq!(app.scroll_offset, 0);
    assert_eq!(app.model, "test-model");
    assert_eq!(app.mode, "act");
    assert_eq!(app.token_count, 0);
    assert_eq!(app.context_window, 0);
    assert_eq!(app.turn_count, 0);
    assert!(app.streaming_text.is_empty());
    assert!(app.input_history.is_empty());
    assert!(app.history_index.is_none());
}

#[test]
fn test_submit_input_empty_returns_none() {
    let mut app = make_app();
    app.input = "   ".to_string();
    assert!(app.submit_input().is_none());
}

#[test]
fn test_submit_input_returns_text_and_resets() {
    let mut app = make_app();
    app.input = "hello world".to_string();
    app.input_cursor = 11;

    let result = app.submit_input();
    assert_eq!(result, Some("hello world".to_string()));
    assert!(app.input.is_empty());
    assert_eq!(app.input_cursor, 0);
    // submit_input no longer adds to messages or history (Inbox handles that)
    assert_eq!(app.messages.len(), 0);
    assert_eq!(app.input_history.len(), 0);
}

#[test]
fn test_submit_input_adds_to_history() {
    let mut app = make_app();
    // submit_input no longer saves history; push_to_inbox does.
    app.push_to_inbox("first command".to_string());
    app.push_to_inbox("second command".to_string());

    assert_eq!(app.input_history.len(), 2);
    assert_eq!(app.input_history[0], "first command");
    assert_eq!(app.input_history[1], "second command");
    assert!(app.history_index.is_none());
}

// --- Inbox tests ---

#[test]
fn test_push_to_inbox() {
    let mut app = make_app();
    app.push_to_inbox("hello".to_string());
    assert_eq!(app.inbox.len(), 1);
    assert_eq!(app.inbox[0], "hello");
    assert_eq!(app.input_history.last(), Some(&"hello".to_string()));
    assert!(app.history_index.is_none());
}

#[test]
fn test_try_forward_when_idle() {
    let mut app = make_app();
    app.agent_idle = true;
    app.inbox.push_back("msg1".to_string());
    let forwarded = app.try_forward_inbox();
    assert_eq!(forwarded, Some("msg1".to_string()));
    assert!(!app.agent_idle);
    assert!(app.inbox.is_empty());
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, "user");
    assert_eq!(app.messages[0].content, "msg1");
}

#[test]
fn test_try_forward_when_busy() {
    let mut app = make_app();
    app.agent_idle = false;
    app.inbox.push_back("msg1".to_string());
    let forwarded = app.try_forward_inbox();
    assert!(forwarded.is_none());
    assert_eq!(app.inbox.len(), 1);
}

#[test]
fn test_pop_inbox_to_input() {
    let mut app = make_app();
    app.inbox.push_back("first".to_string());
    app.inbox.push_back("second".to_string());
    assert!(app.pop_inbox_to_input());
    assert_eq!(app.input, "second");
    assert_eq!(app.input_cursor, 6);
    assert_eq!(app.inbox.len(), 1);
}

#[test]
fn test_pop_inbox_empty_returns_false() {
    let mut app = make_app();
    assert!(!app.pop_inbox_to_input());
}

#[test]
fn test_awaiting_input_sets_idle() {
    let mut app = make_app();
    assert!(!app.agent_idle);
    app.handle_agent_event(AgentEvent::AwaitingInput);
    assert!(app.agent_idle);
}

#[test]
fn test_forward_clears_idle() {
    let mut app = make_app();
    app.agent_idle = true;
    app.inbox.push_back("msg".to_string());
    app.try_forward_inbox();
    assert!(!app.agent_idle);
}
