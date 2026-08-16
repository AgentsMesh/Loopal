use std::sync::Arc;
use std::sync::atomic::Ordering;

use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, InterruptSignal, McpServerSnapshot,
    UserQuestionResponse,
};
use loopal_session::SessionController;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::{mpsc, watch};

use super::*;
use crate::app::{McpPageState, SubPage};

fn app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(8);
    let (permission_tx, _) = mpsc::channel::<bool>(8);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(8);
    let (interrupt_tx, _) = watch::channel(0_u64);
    App::new(
        SessionController::new(
            control_tx,
            permission_tx,
            question_tx,
            InterruptSignal::new(),
            Arc::new(interrupt_tx),
        ),
        std::env::temp_dir(),
    )
}

fn servers() -> Vec<McpServerSnapshot> {
    vec![
        McpServerSnapshot {
            name: "alpha".into(),
            transport: "stdio".into(),
            source: "project".into(),
            status: "connected".into(),
            tool_count: 2,
            resource_count: 1,
            prompt_count: 0,
            errors: Vec::new(),
        },
        McpServerSnapshot {
            name: "beta".into(),
            transport: "streamable-http".into(),
            source: "global".into(),
            status: "failed".into(),
            tool_count: 0,
            resource_count: 0,
            prompt_count: 0,
            errors: vec!["offline".into()],
        },
    ]
}

#[tokio::test]
async fn terminal_runner_restores_cursor_and_returns_exit_state() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut app = app();
    app.exiting = true;
    app.detach_requested = true;
    app.shutdown_initiated = true;
    app.hub_connection_lost.store(true, Ordering::Relaxed);

    let (tx, rx) = mpsc::channel(1);
    tx.send(AppEvent::Tick).await.unwrap();
    let events = EventHandler::from_channel(tx, rx);

    let exit = run_tui_with_terminal(&mut terminal, events, &mut app)
        .await
        .unwrap();
    assert!(exit.detach_requested);
    assert!(exit.connection_lost);
    assert!(exit.shutdown_initiated);
    assert!(exit.reconnect_info.is_none());
}

#[tokio::test]
async fn event_loop_returns_when_every_event_sender_is_closed() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut app = app();
    let (tx, rx) = mpsc::channel(1);
    drop(tx);

    run_tui_loop(
        &mut terminal,
        EventHandler::from_channel(mpsc::channel(1).0, rx),
        &mut app,
    )
    .await
    .unwrap();
}

#[test]
fn mcp_report_refreshes_the_open_page_and_clears_stale_menu_state() {
    let mut app = app();
    let mut page = McpPageState::new(Some(servers()));
    page.selected = 1;
    page.scroll_offset = 1;
    page.open_action_menu();
    assert!(page.action_menu.is_some());
    app.sub_page = Some(SubPage::McpPage(page));

    let report = AgentEvent::root(AgentEventPayload::McpStatusReport {
        servers: vec![servers()[0].clone()],
    });
    assert!(!handle_agent_event(&mut app, report));

    let Some(SubPage::McpPage(page)) = app.sub_page.as_ref() else {
        panic!("MCP page should remain open");
    };
    assert!(page.loaded);
    assert_eq!(page.servers.len(), 1);
    assert_eq!(page.selected, 0);
    assert_eq!(page.scroll_offset, 0);
    assert!(page.action_menu.is_none());
}

#[test]
fn root_resume_warnings_are_displayed_but_child_warnings_are_not() {
    let mut app = app();
    let root_warning = AgentEvent::root(AgentEventPayload::SessionResumeWarnings {
        session_id: "session".into(),
        warnings: vec![
            "task state unavailable".into(),
            "cron state unavailable".into(),
        ],
    });
    assert!(!handle_agent_event(&mut app, root_warning));

    let messages = app.snapshot_conversation("main").messages;
    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages[0].content,
        "Session resume warning: task state unavailable"
    );
    assert_eq!(
        messages[1].content,
        "Session resume warning: cron state unavailable"
    );

    let child_warning = AgentEvent::named(
        "worker",
        AgentEventPayload::SessionResumeWarnings {
            session_id: "child-session".into(),
            warnings: vec!["must stay scoped to child".into()],
        },
    );
    assert!(!handle_agent_event(&mut app, child_warning));
    assert_eq!(app.snapshot_conversation("main").messages.len(), 2);
}

#[test]
fn resumed_event_attempts_root_reload_and_mcp_refresh_is_a_closed_page_noop() {
    let mut app = app();
    let resumed = AgentEvent::root(AgentEventPayload::SessionResumed {
        session_id: "missing-session-for-tui-loop-test".into(),
        message_count: 0,
    });
    assert!(!handle_agent_event(&mut app, resumed));

    let report = AgentEvent::root(AgentEventPayload::McpStatusReport { servers: servers() });
    assert!(!handle_agent_event(&mut app, report));
    assert!(app.sub_page.is_none());
    assert_eq!(app.session.lock().mcp_status.as_ref().unwrap().len(), 2);
}
