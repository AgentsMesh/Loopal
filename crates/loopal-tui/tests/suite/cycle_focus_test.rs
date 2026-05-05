/// Tests for cycle_panel_focus and adjust_agent_scroll (indirect).
use loopal_protocol::{AgentEvent, AgentEventPayload, ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode, PanelKind};
use loopal_tui::dispatch_ops::cycle_panel_focus;

use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn spawn_agent(app: &mut App, name: &str) {
    app.dispatch_event(AgentEvent::named(
        name,
        AgentEventPayload::SubAgentSpawned {
            name: name.to_string(),
            agent_id: format!("id-{name}"),
            parent: Some("main".into()),
            model: Some("test-model".into()),
            session_id: None,
        },
    ));
    app.dispatch_event(AgentEvent::named(name, AgentEventPayload::Started));
}

fn finish_agent(app: &mut App, name: &str) {
    app.dispatch_event(AgentEvent::named(name, AgentEventPayload::Finished));
}

// === Basic cycling ===

#[test]
fn forward_through_agents() {
    let mut app = make_app();
    spawn_agent(&mut app, "a");
    spawn_agent(&mut app, "b");
    spawn_agent(&mut app, "c");
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    cycle_panel_focus(&mut app, true);
    assert_eq!(app.section(PanelKind::Agents).focused.as_deref(), Some("a"));
    cycle_panel_focus(&mut app, true);
    assert_eq!(app.section(PanelKind::Agents).focused.as_deref(), Some("b"));
    cycle_panel_focus(&mut app, true);
    assert_eq!(app.section(PanelKind::Agents).focused.as_deref(), Some("c"));
}

#[test]
fn forward_wraps_around() {
    let mut app = make_app();
    spawn_agent(&mut app, "a");
    spawn_agent(&mut app, "b");
    app.section_mut(PanelKind::Agents).focused = Some("b".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    cycle_panel_focus(&mut app, true);
    assert_eq!(app.section(PanelKind::Agents).focused.as_deref(), Some("a"));
}

#[test]
fn backward_wraps_around() {
    let mut app = make_app();
    spawn_agent(&mut app, "a");
    spawn_agent(&mut app, "b");
    app.section_mut(PanelKind::Agents).focused = Some("a".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    cycle_panel_focus(&mut app, false);
    assert_eq!(app.section(PanelKind::Agents).focused.as_deref(), Some("b"));
}

#[test]
fn backward_from_none_selects_last() {
    let mut app = make_app();
    spawn_agent(&mut app, "a");
    spawn_agent(&mut app, "b");
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    cycle_panel_focus(&mut app, false);
    assert_eq!(app.section(PanelKind::Agents).focused.as_deref(), Some("b"));
}

// === Stale focus recovery ===

#[test]
fn recovers_from_stale_focused_agent() {
    let mut app = make_app();
    spawn_agent(&mut app, "live");
    spawn_agent(&mut app, "dead");
    finish_agent(&mut app, "dead");
    app.section_mut(PanelKind::Agents).focused = Some("dead".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    cycle_panel_focus(&mut app, true);
    assert_eq!(
        app.section(PanelKind::Agents).focused.as_deref(),
        Some("live")
    );
}

// === Empty list auto-exits AgentPanel ===

#[test]
fn empty_list_exits_agent_panel() {
    let mut app = make_app();
    spawn_agent(&mut app, "worker");
    finish_agent(&mut app, "worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    app.section_mut(PanelKind::Agents).scroll_offset = 2;
    cycle_panel_focus(&mut app, true);
    assert!(app.section(PanelKind::Agents).focused.is_none());
    assert_eq!(app.focus_mode, FocusMode::Input);
    assert_eq!(app.section(PanelKind::Agents).scroll_offset, 0);
}

// === Scroll offset (indirect via 7 agents) ===

#[test]
fn scroll_follows_focus_downward() {
    let mut app = make_app();
    for i in 0..7 {
        spawn_agent(&mut app, &format!("a{i}"));
    }
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    for _ in 0..7 {
        cycle_panel_focus(&mut app, true);
    }
    assert_eq!(
        app.section(PanelKind::Agents).focused.as_deref(),
        Some("a6")
    );
    assert!(
        app.section(PanelKind::Agents).scroll_offset >= 2,
        "got {}",
        app.section(PanelKind::Agents).scroll_offset,
    );
}

#[test]
fn scroll_zero_when_few_agents() {
    let mut app = make_app();
    spawn_agent(&mut app, "a");
    spawn_agent(&mut app, "b");
    spawn_agent(&mut app, "c");
    for _ in 0..3 {
        cycle_panel_focus(&mut app, true);
    }
    assert_eq!(app.section(PanelKind::Agents).scroll_offset, 0);
}

#[test]
fn scroll_resets_on_wrap_to_first() {
    let mut app = make_app();
    for i in 0..7 {
        spawn_agent(&mut app, &format!("a{i}"));
    }
    app.section_mut(PanelKind::Agents).focused = Some("a6".into());
    app.section_mut(PanelKind::Agents).scroll_offset = 2;
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    cycle_panel_focus(&mut app, true);
    assert_eq!(
        app.section(PanelKind::Agents).focused.as_deref(),
        Some("a0")
    );
    assert_eq!(app.section(PanelKind::Agents).scroll_offset, 0);
}

#[test]
fn scroll_adjusts_on_backward_past_window() {
    let mut app = make_app();
    for i in 0..7 {
        spawn_agent(&mut app, &format!("a{i}"));
    }
    app.section_mut(PanelKind::Agents).focused = Some("a3".into());
    app.section_mut(PanelKind::Agents).scroll_offset = 2; // window: a2..a6
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);
    cycle_panel_focus(&mut app, false);
    // a3 → a2, still in window
    assert_eq!(
        app.section(PanelKind::Agents).focused.as_deref(),
        Some("a2")
    );
    assert_eq!(app.section(PanelKind::Agents).scroll_offset, 2);
    cycle_panel_focus(&mut app, false);
    // a2 → a1, now ABOVE window → offset adjusts
    assert_eq!(
        app.section(PanelKind::Agents).focused.as_deref(),
        Some("a1")
    );
    assert!(
        app.section(PanelKind::Agents).scroll_offset <= 1,
        "got {}",
        app.section(PanelKind::Agents).scroll_offset
    );
}
