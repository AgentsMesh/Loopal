/// Tests for key behavior within each FocusMode: navigation, Ctrl+P/N, Delete, Enter.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::{AgentEvent, AgentEventPayload, ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode};
use loopal_tui::input::{InputAction, handle_key};

use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "test-model".into(),
        "act".into(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn spawn_agent(app: &App, name: &str) {
    app.session.handle_event(AgentEvent::named(
        name,
        AgentEventPayload::SubAgentSpawned {
            name: name.to_string(),
            agent_id: "id".into(),
            parent: Some("main".into()),
            model: Some("test-model".into()),
            session_id: None,
        },
    ));
    app.session
        .handle_event(AgentEvent::named(name, AgentEventPayload::Started));
}

// === Up/Down in AgentPanel ===

#[test]
fn down_in_agent_panel_returns_panel_down() {
    let mut app = make_app();
    spawn_agent(&app, "a");
    spawn_agent(&app, "b");
    app.focused_agent = Some("a".into());
    app.focus_mode = FocusMode::AgentPanel;
    let action = handle_key(&mut app, key(KeyCode::Down));
    assert!(matches!(action, InputAction::AgentPanelDown));
}

#[test]
fn up_in_agent_panel_returns_panel_up() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.focused_agent = Some("worker".into());
    app.focus_mode = FocusMode::AgentPanel;
    let action = handle_key(&mut app, key(KeyCode::Up));
    assert!(matches!(action, InputAction::AgentPanelUp));
}

#[test]
fn up_in_input_mode_ignores_agent_panel() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.focused_agent = Some("worker".into());
    app.focus_mode = FocusMode::Input;
    app.content_overflows = true;
    let action = handle_key(&mut app, key(KeyCode::Up));
    assert!(!matches!(action, InputAction::AgentPanelUp));
}

// === Ctrl+P/N mode-aware ===

#[test]
fn ctrl_p_in_agent_panel_navigates_up() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.focused_agent = Some("worker".into());
    app.focus_mode = FocusMode::AgentPanel;
    let action = handle_key(&mut app, ctrl('p'));
    assert!(matches!(action, InputAction::AgentPanelUp));
}

#[test]
fn ctrl_n_in_agent_panel_navigates_down() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.focused_agent = Some("worker".into());
    app.focus_mode = FocusMode::AgentPanel;
    let action = handle_key(&mut app, ctrl('n'));
    assert!(matches!(action, InputAction::AgentPanelDown));
}

#[test]
fn ctrl_p_in_input_mode_does_history() {
    let mut app = make_app();
    app.focus_mode = FocusMode::Input;
    let action = handle_key(&mut app, ctrl('p'));
    // Should NOT be AgentPanelUp — it's history/input navigation
    assert!(!matches!(action, InputAction::AgentPanelUp));
}

// === Delete ===

#[test]
fn delete_in_agent_panel_terminates_agent() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.focused_agent = Some("worker".into());
    app.focus_mode = FocusMode::AgentPanel;
    let action = handle_key(&mut app, key(KeyCode::Delete));
    assert!(matches!(action, InputAction::TerminateFocusedAgent));
}

#[test]
fn delete_in_input_mode_deletes_char() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.focused_agent = Some("worker".into());
    app.focus_mode = FocusMode::Input;
    app.input = "hello".into();
    app.input_cursor = 3;
    let action = handle_key(&mut app, key(KeyCode::Delete));
    assert!(matches!(action, InputAction::None));
    assert_eq!(app.input, "helo");
}

// === Enter ===

#[test]
fn enter_in_agent_panel_returns_enter_agent_view() {
    let mut app = make_app();
    spawn_agent(&app, "researcher");
    app.focused_agent = Some("researcher".into());
    app.focus_mode = FocusMode::AgentPanel;
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::EnterAgentView));
}

#[test]
fn enter_in_input_mode_with_focus_also_drills_in() {
    let mut app = make_app();
    spawn_agent(&app, "researcher");
    app.focused_agent = Some("researcher".into());
    app.focus_mode = FocusMode::Input;
    // Empty input + focused agent → drill in (backward-compat path via editing.rs)
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::EnterAgentView));
}

// === Root agent guard ===

#[test]
fn terminate_guards_root_agent() {
    assert_eq!(loopal_session::ROOT_AGENT, "main");
}
