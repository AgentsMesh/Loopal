//! Tests for panel_tab() with Crons panel participation.

use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, CronJobSnapshot, UserQuestionResponse,
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

fn add_cron(app: &mut App, id: &str, prompt: &str) {
    app.cron_snapshots.push(CronJobSnapshot {
        id: id.into(),
        cron_expr: "*/5 * * * *".into(),
        prompt: prompt.into(),
        recurring: true,
        created_at_unix_ms: 1_700_000_000_000,
        next_fire_unix_ms: Some(1_700_000_000_000),
    });
}

#[test]
fn tab_from_agents_to_crons_when_only_these_have_content() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    add_cron(&mut app, "c1", "daily report");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);

    panel_tab(&mut app);

    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Crons));
    assert_eq!(app.section(PanelKind::Crons).focused.as_deref(), Some("c1"));
}

#[test]
fn tab_cycles_through_all_four_panels() {
    use loopal_protocol::{BgTaskSnapshot, BgTaskStatus, TaskSnapshot, TaskSnapshotStatus};
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.task_snapshots.push(TaskSnapshot {
        id: "t1".into(),
        subject: "Task".into(),
        active_form: None,
        status: TaskSnapshotStatus::InProgress,
        blocked_by: Vec::new(),
    });
    app.bg_snapshots.push(BgTaskSnapshot {
        id: "b1".into(),
        description: "bg".into(),
        status: BgTaskStatus::Running,
        exit_code: None,
    });
    add_cron(&mut app, "c1", "cron");

    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);

    panel_tab(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Tasks));
    panel_tab(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::BgTasks));
    panel_tab(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Crons));
    panel_tab(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
}

#[test]
fn tab_skips_empty_crons_panel() {
    use loopal_protocol::{BgTaskSnapshot, BgTaskStatus};
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.bg_snapshots.push(BgTaskSnapshot {
        id: "b1".into(),
        description: "bg".into(),
        status: BgTaskStatus::Running,
        exit_code: None,
    });
    // no cron snapshots
    app.section_mut(PanelKind::BgTasks).focused = Some("b1".into());
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);

    panel_tab(&mut app);
    assert_eq!(app.focus_mode, FocusMode::Panel(PanelKind::Agents));
}
