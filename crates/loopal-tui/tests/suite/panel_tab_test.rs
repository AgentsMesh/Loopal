/// Tests for panel_tab() dispatch: Tab key behavior within the panel zone.
use loopal_protocol::{
    AgentEvent, AgentEventPayload, BgTaskSnapshot, BgTaskStatus, ControlCommand,
    UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode, PanelKind};
use loopal_tui::dispatch_ops::panel_tab;

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

fn add_bg_snapshot(app: &mut App, id: &str, desc: &str) {
    app.bg_snapshots.push(BgTaskSnapshot {
        id: id.into(),
        description: desc.into(),
        status: BgTaskStatus::Running,
        exit_code: None,
    });
}

// === Both panels have content: Tab switches between them ===

#[test]
fn tab_switches_from_agents_to_bg_tasks() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    add_bg_snapshot(&mut app, "t1", "build");
    app.focused_agent = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);

    panel_tab(&mut app);

    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::BgTasks));
    assert!(
        app.focused_bg_task.is_some(),
        "focused_bg_task should be set"
    );
}

#[test]
fn tab_switches_from_bg_tasks_to_agents() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    add_bg_snapshot(&mut app, "t1", "build");
    app.focused_bg_task = Some("t1".into());
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);

    panel_tab(&mut app);

    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
    assert!(app.focused_agent.is_some(), "focused_agent should be set");
}

// === Only one panel: Tab cycles within that panel ===

#[test]
fn tab_cycles_agents_when_only_agents() {
    let mut app = make_app();
    spawn_agent(&app, "a");
    spawn_agent(&app, "b");
    app.focused_agent = Some("a".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);

    panel_tab(&mut app);

    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
    assert_eq!(
        app.focused_agent.as_deref(),
        Some("b"),
        "should cycle to next agent"
    );
}

#[test]
fn tab_cycles_bg_tasks_when_only_bg_tasks() {
    let mut app = make_app();
    add_bg_snapshot(&mut app, "t1", "lint");
    add_bg_snapshot(&mut app, "t2", "test");
    app.focused_bg_task = Some("t1".into());
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);

    panel_tab(&mut app);

    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::BgTasks));
    assert_eq!(
        app.focused_bg_task.as_deref(),
        Some("t2"),
        "should cycle to next task"
    );
}

// === Round-trip ===

#[test]
fn tab_roundtrip_both_panels() {
    let mut app = make_app();
    spawn_agent(&app, "alpha");
    add_bg_snapshot(&mut app, "t1", "deploy");
    app.focused_agent = Some("alpha".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);

    panel_tab(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::BgTasks));

    panel_tab(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
    assert!(app.focused_agent.is_some());
}

// === Edge case: single element wraps to itself ===

#[test]
fn tab_noop_when_single_agent() {
    let mut app = make_app();
    spawn_agent(&app, "solo");
    app.focused_agent = Some("solo".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);

    panel_tab(&mut app);

    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
    assert_eq!(
        app.focused_agent.as_deref(),
        Some("solo"),
        "single agent wraps to itself"
    );
}
