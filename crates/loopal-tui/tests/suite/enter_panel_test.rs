/// Tests for enter_panel dispatch function.
use loopal_protocol::{
    AgentEvent, AgentEventPayload, BgTaskSnapshot, BgTaskStatus, ControlCommand,
    TaskSnapshot, TaskSnapshotStatus, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode, PanelKind};
use loopal_tui::dispatch_ops::enter_panel;

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

fn add_bg_snapshot(app: &mut App, id: &str, desc: &str) {
    app.bg_snapshots.push(BgTaskSnapshot {
        id: id.into(),
        description: desc.into(),
        status: BgTaskStatus::Running,
        exit_code: None,
    });
}

fn add_task_snapshot(app: &mut App, id: &str, subject: &str) {
    app.task_snapshots.push(TaskSnapshot {
        id: id.into(),
        subject: subject.into(),
        active_form: None,
        status: TaskSnapshotStatus::InProgress,
        blocked_by: Vec::new(),
    });
}

#[test]
fn noop_without_agents() {
    let mut app = make_app();
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Input);
    assert!(app.section(PanelKind::Agents).focused.is_none());
}

#[test]
fn sets_mode_and_focuses_first() {
    let mut app = make_app();
    spawn_agent(&app, "alpha");
    spawn_agent(&app, "beta");
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
    assert_eq!(app.section(PanelKind::Agents).focused.as_deref(), Some("alpha"));
}

#[test]
fn keeps_existing_live_focus() {
    let mut app = make_app();
    spawn_agent(&app, "alpha");
    spawn_agent(&app, "beta");
    app.section_mut(PanelKind::Agents).focused = Some("beta".into());
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
    assert_eq!(app.section(PanelKind::Agents).focused.as_deref(), Some("beta"));
}

#[test]
fn refocuses_when_focused_agent_is_dead() {
    let mut app = make_app();
    spawn_agent(&app, "alive");
    spawn_agent(&app, "dead");
    finish_agent(&app, "dead");
    app.section_mut(PanelKind::Agents).focused = Some("dead".into());
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
    assert_eq!(
        app.section(PanelKind::Agents).focused.as_deref(),
        Some("alive"),
        "should re-focus to a live agent"
    );
}

#[test]
fn noop_when_only_finished_agents() {
    let mut app = make_app();
    spawn_agent(&app, "done");
    finish_agent(&app, "done");
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Input);
}

#[test]
fn enters_bg_tasks_when_no_agents_but_bg_tasks() {
    let mut app = make_app();
    add_bg_snapshot(&mut app, "t1", "compiling");
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::BgTasks));
    assert!(app.section(PanelKind::BgTasks).focused.is_some(), "should set focused_bg_task");
}

#[test]
fn enters_bg_tasks_when_only_finished_agents_and_bg_tasks() {
    let mut app = make_app();
    spawn_agent(&app, "done");
    finish_agent(&app, "done");
    add_bg_snapshot(&mut app, "t2", "testing");
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::BgTasks));
}

#[test]
fn prefers_agents_over_bg_tasks() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    add_bg_snapshot(&mut app, "t3", "linting");
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
}

#[test]
fn enters_tasks_when_no_agents_but_tasks() {
    let mut app = make_app();
    add_task_snapshot(&mut app, "1", "Build thing");
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Tasks));
    assert!(app.section(PanelKind::Tasks).focused.is_some());
}

#[test]
fn prefers_agents_over_tasks() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    add_task_snapshot(&mut app, "1", "Build thing");
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
}

#[test]
fn enters_tasks_before_bg_tasks() {
    let mut app = make_app();
    add_task_snapshot(&mut app, "1", "Task A");
    add_bg_snapshot(&mut app, "bg1", "compiling");
    enter_panel(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Tasks));
}
