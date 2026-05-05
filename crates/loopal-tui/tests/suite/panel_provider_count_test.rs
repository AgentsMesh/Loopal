//! Tests for `PanelProvider::count` across all 4 providers.
//!
//! Each provider overrides `count` with an allocation-free `.filter().count()`
//! variant (the section header only needs the integer). These tests
//! verify the override matches the length of `item_ids` — so if someone
//! changes the filter predicate in one place and forgets the other, the
//! test fails.

use loopal_protocol::{
    AgentEvent, AgentEventPayload, BgTaskDetail, BgTaskStatus, ControlCommand, CronJobSnapshot,
    TaskSnapshot, TaskSnapshotStatus, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::{App, PanelKind};
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
            model: Some("m".into()),
            session_id: None,
        },
    ));
    app.dispatch_event(AgentEvent::named(name, AgentEventPayload::Started));
}

fn assert_count_matches_item_ids(
    app: &App,
    state: &loopal_session::state::SessionState,
    kind: PanelKind,
) {
    let provider = app.panel_registry.by_kind(kind).unwrap();
    let expected = provider.item_ids(app, state).len();
    assert_eq!(
        provider.count(app, state),
        expected,
        "count / item_ids.len drift for {kind:?}"
    );
}

// ── Agent provider ──────────────────────────────────────────────────

#[test]
fn agent_count_matches_live_agents() {
    let mut app = make_app();
    spawn_agent(&mut app, "w1");
    spawn_agent(&mut app, "w2");
    spawn_agent(&mut app, "w3");
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::Agents).unwrap();
    assert_eq!(provider.count(&app, &state), 3);
    assert_count_matches_item_ids(&app, &state, PanelKind::Agents);
}

#[test]
fn agent_count_zero_when_no_live_agents() {
    let app = make_app();
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::Agents).unwrap();
    assert_eq!(provider.count(&app, &state), 0);
}

#[test]
fn agent_count_does_not_require_extra_lock() {
    // The caller holds the guard; `count` must read from the passed-in
    // `state` without re-acquiring. Success = no deadlock.
    let mut app = make_app();
    spawn_agent(&mut app, "w1");
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::Agents).unwrap();
    let _ = provider.count(&app, &state);
}

// ── Tasks provider ──────────────────────────────────────────────────

#[test]
fn tasks_count_matches_snapshots() {
    let app = make_app();
    app.view_clients["main"].inject_tasks_for_test(vec![
        TaskSnapshot {
            id: "1".into(),
            subject: "a".into(),
            active_form: None,
            status: TaskSnapshotStatus::Pending,
            blocked_by: Vec::new(),
        },
        TaskSnapshot {
            id: "2".into(),
            subject: "b".into(),
            active_form: None,
            status: TaskSnapshotStatus::InProgress,
            blocked_by: Vec::new(),
        },
        TaskSnapshot {
            id: "3".into(),
            subject: "c".into(),
            active_form: None,
            status: TaskSnapshotStatus::Completed,
            blocked_by: Vec::new(),
        },
    ]);
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::Tasks).unwrap();
    // Completed excluded by tasks_panel::task_ids.
    assert_eq!(provider.count(&app, &state), 2);
    assert_count_matches_item_ids(&app, &state, PanelKind::Tasks);
}

// ── BgTasks provider ────────────────────────────────────────────────

#[test]
fn bg_tasks_count_matches_running_snapshots() {
    let app = make_app();
    app.view_clients["main"].inject_bg_for_test(vec![
        BgTaskDetail {
            id: "bg_1".into(),
            description: "a".into(),
            status: BgTaskStatus::Running,
            exit_code: None,
            output: String::new(),
        }
        .to_snapshot(),
        BgTaskDetail {
            id: "bg_2".into(),
            description: "b".into(),
            status: BgTaskStatus::Completed,
            exit_code: Some(0),
            output: String::new(),
        }
        .to_snapshot(),
    ]);
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::BgTasks).unwrap();
    // Completed excluded by bg_tasks_panel::task_ids.
    assert_eq!(provider.count(&app, &state), 1);
    assert_count_matches_item_ids(&app, &state, PanelKind::BgTasks);
}

// ── Crons provider ──────────────────────────────────────────────────

#[test]
fn crons_count_matches_snapshots() {
    let app = make_app();
    app.view_clients["main"].inject_crons_for_test(
        (0..3)
            .map(|i| CronJobSnapshot {
                id: format!("c{i}"),
                cron_expr: "*/5 * * * *".into(),
                prompt: "p".into(),
                recurring: true,
                created_at_unix_ms: 0,
                next_fire_unix_ms: None,
                durable: false,
            })
            .collect(),
    );
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::Crons).unwrap();
    assert_eq!(provider.count(&app, &state), 3);
    assert_count_matches_item_ids(&app, &state, PanelKind::Crons);
}
