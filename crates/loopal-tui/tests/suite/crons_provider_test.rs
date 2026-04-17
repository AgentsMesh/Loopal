//! Direct tests for `CronsPanelProvider` — asserts each PanelProvider trait
//! method returns the expected values. Provider is accessed via the
//! `App.panel_registry.by_kind` lookup, so the module stays non-pub.

use loopal_protocol::{ControlCommand, CronJobSnapshot, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, PanelKind};
use loopal_tui::views::crons_panel::MAX_CRON_VISIBLE;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;

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

fn snap(id: &str, prompt: &str, recurring: bool) -> CronJobSnapshot {
    CronJobSnapshot {
        id: id.into(),
        cron_expr: "*/5 * * * *".into(),
        prompt: prompt.into(),
        recurring,
        created_at_unix_ms: 1_700_000_000_000,
        next_fire_unix_ms: Some(1_700_000_000_000),
    }
}

#[test]
fn provider_kind_is_crons() {
    let app = make_app();
    let provider = app
        .panel_registry
        .by_kind(PanelKind::Crons)
        .expect("crons provider registered");
    assert_eq!(provider.kind(), PanelKind::Crons);
}

#[test]
fn provider_max_visible_matches_panel_constant() {
    let app = make_app();
    let provider = app.panel_registry.by_kind(PanelKind::Crons).unwrap();
    assert_eq!(provider.max_visible(), MAX_CRON_VISIBLE);
}

#[test]
fn provider_item_ids_empty_when_no_snapshots() {
    let app = make_app();
    let provider = app.panel_registry.by_kind(PanelKind::Crons).unwrap();
    assert!(provider.item_ids(&app).is_empty());
}

#[test]
fn provider_item_ids_lists_all_snapshots_in_order() {
    let mut app = make_app();
    app.cron_snapshots = vec![
        snap("first", "p1", true),
        snap("second", "p2", false),
        snap("third", "p3", true),
    ];
    let provider = app.panel_registry.by_kind(PanelKind::Crons).unwrap();
    assert_eq!(provider.item_ids(&app), vec!["first", "second", "third"]);
}

#[test]
fn provider_height_zero_when_empty() {
    let app = make_app();
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::Crons).unwrap();
    assert_eq!(provider.height(&app, &state), 0);
}

#[test]
fn provider_height_counts_snapshots() {
    let mut app = make_app();
    app.cron_snapshots = (0..3).map(|i| snap(&format!("id{i}"), "p", true)).collect();
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::Crons).unwrap();
    assert_eq!(provider.height(&app, &state), 3);
}

#[test]
fn provider_height_capped_at_max_visible() {
    let mut app = make_app();
    app.cron_snapshots = (0..20)
        .map(|i| snap(&format!("id{i}"), "p", true))
        .collect();
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::Crons).unwrap();
    assert_eq!(provider.height(&app, &state) as usize, MAX_CRON_VISIBLE);
}

#[test]
fn provider_render_no_panic_with_focus_and_offset() {
    let mut app = make_app();
    app.cron_snapshots = (0..6)
        .map(|i| snap(&format!("id{i:02}"), "prompt", true))
        .collect();
    app.section_mut(PanelKind::Crons).scroll_offset = 2;
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::Crons).unwrap();

    let backend = TestBackend::new(80, 4);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 4);
            provider.render(
                f,
                &app,
                &state,
                Some("id03"),
                std::time::Duration::ZERO,
                area,
            );
        })
        .unwrap();
}

#[test]
fn provider_render_empty_area_is_noop() {
    let mut app = make_app();
    app.cron_snapshots = vec![snap("x", "y", true)];
    let state = app.session.lock();
    let provider = app.panel_registry.by_kind(PanelKind::Crons).unwrap();

    let backend = TestBackend::new(80, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 0);
            provider.render(f, &app, &state, None, std::time::Duration::ZERO, area);
        })
        .unwrap();
}
