/// Tests for FocusMode transitions: entering/exiting AgentPanel, auto-switch on typing.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::{AgentEvent, AgentEventPayload, ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode, PanelKind};
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

// === Default state ===

#[test]
fn default_focus_mode_is_input() {
    let app = make_app();
    assert_eq!(app.focus_mode, FocusMode::Input);
}

// === Tab → enter/exit AgentPanel ===

#[test]
fn tab_returns_enter_agent_panel() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    let action = handle_key(&mut app, key(KeyCode::Tab));
    assert!(matches!(action, InputAction::EnterPanel));
}

#[test]
fn tab_without_agents_still_returns_enter_but_mode_unchanged() {
    let mut app = make_app();
    let action = handle_key(&mut app, key(KeyCode::Tab));
    assert!(matches!(action, InputAction::EnterPanel));
    assert_eq!(app.focus_mode, FocusMode::Input);
}

#[test]
fn tab_in_panel_returns_panel_tab() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    let action = handle_key(&mut app, key(KeyCode::Tab));
    assert!(matches!(action, InputAction::PanelTab));
}

#[test]
fn esc_exits_agent_panel() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert!(matches!(action, InputAction::ExitPanel));
}

#[test]
fn esc_in_agent_panel_takes_priority_over_view_exit() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.session.enter_agent_view("worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert!(matches!(action, InputAction::ExitPanel));
}

// === Char/Backspace auto-switch from AgentPanel → Input ===

#[test]
fn char_in_agent_panel_switches_to_input_and_inserts() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    let action = handle_key(&mut app, key(KeyCode::Char('x')));
    assert!(matches!(action, InputAction::None));
    assert_eq!(app.focus_mode, FocusMode::Input);
    assert_eq!(app.input, "x");
}

#[test]
fn backspace_in_agent_panel_switches_to_input() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    app.input = "hi".into();
    app.input_cursor = 2;
    let action = handle_key(&mut app, key(KeyCode::Backspace));
    assert!(matches!(action, InputAction::None));
    assert_eq!(app.focus_mode, FocusMode::Input);
    assert_eq!(app.input, "h");
}

// === Ctrl+C priority chain ===

#[test]
fn ctrl_c_clears_input_first_even_in_agent_panel() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    app.input = "text".into();
    handle_key(&mut app, ctrl('c'));
    assert!(app.input.is_empty());
    assert!(app.section(PanelKind::Agents).focused.is_some(), "focus not cleared yet");
    assert_eq!(
        app.focus_mode,
        FocusMode::Panel(PanelKind::Agents),
        "mode unchanged"
    );
}

#[test]
fn ctrl_c_exits_agent_panel_and_clears_focus() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    app.section_mut(PanelKind::Agents).scroll_offset = 3;
    handle_key(&mut app, ctrl('c'));
    assert!(app.section(PanelKind::Agents).focused.is_none());
    assert_eq!(app.focus_mode, FocusMode::Input);
    assert_eq!(app.section(PanelKind::Agents).scroll_offset, 0);
}

#[test]
fn ctrl_c_clears_focus_in_input_mode() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    handle_key(&mut app, ctrl('c'));
    assert!(app.section(PanelKind::Agents).focused.is_none());
    assert_eq!(app.focus_mode, FocusMode::Input);
}

#[test]
fn ctrl_c_noop_when_idle_no_focus_no_input() {
    let mut app = make_app();
    app.session
        .handle_event(AgentEvent::named("main", AgentEventPayload::AwaitingInput));
    let action = handle_key(&mut app, ctrl('c'));
    assert!(matches!(action, InputAction::None));
}

#[test]
fn ctrl_c_from_tasks_panel_preserves_agent_focus() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.section_mut(PanelKind::Tasks).focused = Some("1".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Tasks);
    handle_key(&mut app, ctrl('c'));
    assert!(app.section(PanelKind::Tasks).focused.is_none(), "active panel cleared");
    assert_eq!(
        app.section(PanelKind::Agents).focused.as_deref(),
        Some("worker"),
        "other panel preserved"
    );
}
