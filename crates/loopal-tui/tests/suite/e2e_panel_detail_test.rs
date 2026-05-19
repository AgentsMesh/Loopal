//! Full-stack TUI e2e tests for panel detail sub-pages.
//!
//! Drives the complete chain: TUI dispatch → SessionController.send_control
//! → mpsc<ControlCommand> → runtime → backend resource ops. The cron_bridge
//! / bg_task_bridge are not wired in the test harness, so view-state
//! propagation is modeled via dispatch_event after the runtime acts.

use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{
    AgentEvent, AgentEventPayload, BgTaskSnapshot, BgTaskStatus, TaskSnapshot, TaskSnapshotStatus,
};
use loopal_scheduler::CronScheduler;
use loopal_test_support::HarnessBuilder;
use loopal_tui::app::{App, PanelKind, SubPage};
use loopal_tui::input::InputAction;
use loopal_tui::key_dispatch_for_test::dispatch;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

fn wrap_tui(inner: loopal_test_support::SpawnedHarness) -> TuiTestHarness {
    let terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    let app = App::new(
        inner.session_ctrl.clone(),
        inner.fixture.path().to_path_buf(),
    );
    TuiTestHarness {
        terminal,
        app,
        inner,
    }
}

async fn wait_for_empty_scheduler(scheduler: &CronScheduler) {
    for _ in 0..50 {
        if scheduler.list().await.is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("scheduler did not drain CronDelete within timeout");
}

#[tokio::test]
async fn cron_detail_x_removes_cron_via_full_stack() {
    let scheduler = Arc::new(CronScheduler::new());
    let cron_id = scheduler
        .add("* * * * *", "ping prod", true, false)
        .await
        .expect("seed cron");

    let inner = HarnessBuilder::new()
        .messages(vec![])
        .scheduler(scheduler.clone())
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);
    let _ = harness.collect_until_idle().await;

    let snapshots: Vec<_> = scheduler
        .list()
        .await
        .into_iter()
        .map(loopal_agent::cron_info_to_snapshot)
        .collect();
    harness
        .app
        .dispatch_event(AgentEvent::root(AgentEventPayload::CronsChanged {
            crons: snapshots,
        }));
    assert_eq!(
        harness.app.view_client_for("main").cron_snapshots().len(),
        1
    );

    harness.app.section_mut(PanelKind::Crons).focused = Some(cron_id.clone());
    dispatch(&mut harness.app, InputAction::EnterCronView).await;
    assert!(matches!(harness.app.sub_page, Some(SubPage::CronDetail(_))));

    let text = harness.render_text();
    assert!(
        text.contains("ping prod"),
        "CronDetail must render the cron prompt"
    );
    assert!(
        text.contains(&cron_id[..8]),
        "CronDetail must render the id"
    );

    dispatch(&mut harness.app, InputAction::StopFocusedSubPageItem).await;
    assert!(harness.app.sub_page.is_none(), "x closes sub-page eagerly");

    wait_for_empty_scheduler(&scheduler).await;

    harness
        .app
        .dispatch_event(AgentEvent::root(AgentEventPayload::CronsChanged {
            crons: Vec::new(),
        }));
    assert!(
        harness
            .app
            .view_client_for("main")
            .cron_snapshots()
            .is_empty()
    );
    let text = harness.render_text();
    assert!(!text.contains("ping prod"), "deleted cron must vanish");
}

#[tokio::test]
async fn bg_task_kill_against_missing_id_does_not_panic_runtime() {
    let inner = HarnessBuilder::new().messages(vec![]).build_spawned().await;
    let mut harness = wrap_tui(inner);
    let _ = harness.collect_until_idle().await;

    harness
        .app
        .view_client_for("main")
        .inject_bg_for_test(vec![BgTaskSnapshot {
            id: "bg_phantom".into(),
            description: "synthetic".into(),
            status: BgTaskStatus::Running,
            exit_code: None,
            created_at_unix_ms: 0,
        }]);

    harness.app.section_mut(PanelKind::BgTasks).focused = Some("bg_phantom".into());
    dispatch(&mut harness.app, InputAction::EnterBgTaskView).await;
    dispatch(&mut harness.app, InputAction::StopFocusedSubPageItem).await;

    assert!(harness.app.sub_page.is_none(), "sub_page closes eagerly");

    // Let runtime drain the BgTaskKill command; bg_stop returns an error
    // for an unknown id but must not panic the agent loop. The handler
    // doesn't emit any event, so we yield + sleep rather than wait for
    // an event signal.
    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Reachability check: hub channel still alive — the agent loop can
    // still receive control commands after the bg kill.
    harness
        .inner
        .control_tx
        .send(loopal_protocol::ControlCommand::Compact { instructions: None })
        .await
        .expect("control channel must remain open after bg kill");
}

#[tokio::test]
async fn task_detail_renders_description_blocks_and_blocked_by() {
    let inner = HarnessBuilder::new().messages(vec![]).build_spawned().await;
    let mut harness = wrap_tui(inner);
    let _ = harness.collect_until_idle().await;

    let task = TaskSnapshot {
        id: "42".into(),
        subject: "Wire DB".into(),
        active_form: None,
        status: TaskSnapshotStatus::Pending,
        blocked_by: vec!["8".into()],
        description: "Implement the connection pool".into(),
        blocks: vec!["50".into(), "51".into()],
    };
    harness
        .app
        .dispatch_event(AgentEvent::root(AgentEventPayload::TasksChanged {
            tasks: vec![task],
        }));

    harness.app.section_mut(PanelKind::Tasks).focused = Some("42".into());
    dispatch(&mut harness.app, InputAction::EnterTaskView).await;
    assert!(matches!(harness.app.sub_page, Some(SubPage::TaskDetail(_))));

    let text = harness.render_text();
    assert!(text.contains("Wire DB"), "subject missing");
    assert!(
        text.contains("Implement the connection pool"),
        "description missing — TaskSnapshot.description not threaded to render"
    );
    assert!(text.contains("50"), "blocks id 50 missing");
    assert!(text.contains("51"), "blocks id 51 missing");
    assert!(text.contains("8"), "blocked_by id 8 missing");
}
