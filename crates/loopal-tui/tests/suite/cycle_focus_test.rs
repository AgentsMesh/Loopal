/// Tests for cycle_agent_focus and adjust_agent_scroll (indirect).
use loopal_protocol::{AgentEvent, AgentEventPayload, ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode};
use loopal_tui::dispatch_ops::cycle_agent_focus;

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

fn spawn_agent(app: &App, name: &str) {
    app.session.handle_event(AgentEvent::named(
        name,
        AgentEventPayload::SubAgentSpawned {
            name: name.to_string(),
            agent_id: format!("id-{name}"),
            parent: Some("main".into()),
            model: Some("test-model".into()),
            session_id: None,
        },
    ));
    app.session
        .handle_event(AgentEvent::named(name, AgentEventPayload::Started));
}

fn finish_agent(app: &App, name: &str) {
    app.session
        .handle_event(AgentEvent::named(name, AgentEventPayload::Finished));
}

// === Basic cycling ===

#[test]
fn forward_through_agents() {
    let mut app = make_app();
    spawn_agent(&app, "a");
    spawn_agent(&app, "b");
    spawn_agent(&app, "c");
    app.focus_mode = FocusMode::AgentPanel;
    cycle_agent_focus(&mut app, true);
    assert_eq!(app.focused_agent.as_deref(), Some("a"));
    cycle_agent_focus(&mut app, true);
    assert_eq!(app.focused_agent.as_deref(), Some("b"));
    cycle_agent_focus(&mut app, true);
    assert_eq!(app.focused_agent.as_deref(), Some("c"));
}

#[test]
fn forward_wraps_around() {
    let mut app = make_app();
    spawn_agent(&app, "a");
    spawn_agent(&app, "b");
    app.focused_agent = Some("b".into());
    cycle_agent_focus(&mut app, true);
    assert_eq!(app.focused_agent.as_deref(), Some("a"));
}

#[test]
fn backward_wraps_around() {
    let mut app = make_app();
    spawn_agent(&app, "a");
    spawn_agent(&app, "b");
    app.focused_agent = Some("a".into());
    cycle_agent_focus(&mut app, false);
    assert_eq!(app.focused_agent.as_deref(), Some("b"));
}

#[test]
fn backward_from_none_selects_last() {
    let mut app = make_app();
    spawn_agent(&app, "a");
    spawn_agent(&app, "b");
    cycle_agent_focus(&mut app, false);
    assert_eq!(app.focused_agent.as_deref(), Some("b"));
}

// === Stale focus recovery ===

#[test]
fn recovers_from_stale_focused_agent() {
    let mut app = make_app();
    spawn_agent(&app, "live");
    spawn_agent(&app, "dead");
    finish_agent(&app, "dead");
    app.focused_agent = Some("dead".into());
    cycle_agent_focus(&mut app, true);
    assert_eq!(app.focused_agent.as_deref(), Some("live"));
}

// === Empty list auto-exits AgentPanel ===

#[test]
fn empty_list_exits_agent_panel() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    finish_agent(&app, "worker");
    app.focused_agent = Some("worker".into());
    app.focus_mode = FocusMode::AgentPanel;
    app.agent_panel_offset = 2;
    cycle_agent_focus(&mut app, true);
    assert!(app.focused_agent.is_none());
    assert_eq!(app.focus_mode, FocusMode::Input);
    assert_eq!(app.agent_panel_offset, 0);
}

// === Scroll offset (indirect via 7 agents) ===

#[test]
fn scroll_follows_focus_downward() {
    let mut app = make_app();
    for i in 0..7 {
        spawn_agent(&app, &format!("a{i}"));
    }
    app.focus_mode = FocusMode::AgentPanel;
    for _ in 0..7 {
        cycle_agent_focus(&mut app, true);
    }
    assert_eq!(app.focused_agent.as_deref(), Some("a6"));
    assert!(
        app.agent_panel_offset >= 2,
        "got {}",
        app.agent_panel_offset,
    );
}

#[test]
fn scroll_zero_when_few_agents() {
    let mut app = make_app();
    spawn_agent(&app, "a");
    spawn_agent(&app, "b");
    spawn_agent(&app, "c");
    for _ in 0..3 {
        cycle_agent_focus(&mut app, true);
    }
    assert_eq!(app.agent_panel_offset, 0);
}

#[test]
fn scroll_resets_on_wrap_to_first() {
    let mut app = make_app();
    for i in 0..7 {
        spawn_agent(&app, &format!("a{i}"));
    }
    app.focused_agent = Some("a6".into());
    app.agent_panel_offset = 2;
    cycle_agent_focus(&mut app, true);
    assert_eq!(app.focused_agent.as_deref(), Some("a0"));
    assert_eq!(app.agent_panel_offset, 0);
}

#[test]
fn scroll_adjusts_on_backward_past_window() {
    let mut app = make_app();
    for i in 0..7 {
        spawn_agent(&app, &format!("a{i}"));
    }
    app.focused_agent = Some("a3".into());
    app.agent_panel_offset = 2; // window: a2..a6
    cycle_agent_focus(&mut app, false);
    // a3 → a2, still in window
    assert_eq!(app.focused_agent.as_deref(), Some("a2"));
    assert_eq!(app.agent_panel_offset, 2);
    cycle_agent_focus(&mut app, false);
    // a2 → a1, now ABOVE window → offset adjusts
    assert_eq!(app.focused_agent.as_deref(), Some("a1"));
    assert!(
        app.agent_panel_offset <= 1,
        "got {}",
        app.agent_panel_offset
    );
}
