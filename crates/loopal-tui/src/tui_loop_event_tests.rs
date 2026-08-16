use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, InterruptSignal, UserQuestionResponse,
    WorkflowNodeId, WorkflowRunId, WorkflowRunState, WorkflowRunSummary, WorkflowStateCounts,
};
use loopal_session::SessionController;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::{Semaphore, mpsc, watch};

use super::*;
use crate::input::paste::PasteResult;

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

fn key(code: KeyCode, modifiers: KeyModifiers) -> AppEvent {
    AppEvent::Key(KeyEvent::new(code, modifiers))
}

fn workflow(revision: u64, state: WorkflowRunState) -> WorkflowRunSummary {
    WorkflowRunSummary {
        id: WorkflowRunId::new("wrun_loop"),
        run_goal: "exercise loop resync".into(),
        state,
        revision,
        output_node: WorkflowNodeId::new("done"),
        counts: WorkflowStateCounts {
            pending: 0,
            ready: 0,
            active: 0,
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        },
        created_at_unix_ms: 1,
        updated_at_unix_ms: revision,
    }
}

#[tokio::test]
async fn event_loop_drains_every_event_variant_before_quitting() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut app = app();
    let (tx, rx) = mpsc::channel(16);
    for event in [
        AppEvent::ScrollUp,
        AppEvent::ScrollDown,
        key(KeyCode::Char('k'), KeyModifiers::NONE),
        AppEvent::Agent(Box::new(AgentEvent::root(AgentEventPayload::Started))),
        AppEvent::Paste(PasteResult::Text("paste".into())),
        AppEvent::Resize(100, 40),
        AppEvent::Tick,
        key(KeyCode::Char('d'), KeyModifiers::CONTROL),
    ] {
        tx.try_send(event).unwrap();
    }

    let events = EventHandler::from_channel(tx, rx);
    run_tui_loop_with_animation_clock(&mut terminal, events, &mut app, || {
        std::time::Duration::from_millis(7)
    })
    .await
    .unwrap();

    assert!(app.exiting);
    assert_eq!(app.input, "kpaste");
    assert_eq!(
        app.view_clients["main"]
            .state()
            .state()
            .agent
            .observable
            .status,
        loopal_protocol::AgentStatus::Running
    );
}

#[tokio::test]
async fn resync_redraws_before_a_later_quit_event() {
    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    let mut app = app();
    let (tx, rx) = mpsc::channel(4);
    tx.try_send(AppEvent::Resync).unwrap();

    let second_draw = Arc::new(Semaphore::new(0));
    let sender_gate = Arc::clone(&second_draw);
    let sender = tx.clone();
    let quit = tokio::spawn(async move {
        let permit = sender_gate.acquire().await.unwrap();
        permit.forget();
        sender
            .send(key(KeyCode::Char('d'), KeyModifiers::CONTROL))
            .await
            .unwrap();
    });
    let draws = Arc::new(AtomicUsize::new(0));
    let clock_draws = Arc::clone(&draws);
    let clock_gate = Arc::clone(&second_draw);

    let events = EventHandler::from_channel(tx, rx);
    run_tui_loop_with_animation_clock(&mut terminal, events, &mut app, move || {
        if clock_draws.fetch_add(1, Ordering::Relaxed) == 1 {
            clock_gate.add_permits(1);
        }
        std::time::Duration::ZERO
    })
    .await
    .unwrap();
    quit.await.unwrap();

    assert!(draws.load(Ordering::Relaxed) >= 2);
    assert!(app.exiting);
}

#[test]
fn workflow_revision_gap_is_propagated_to_the_event_loop() {
    let mut app = app();
    assert!(!handle_agent_event(
        &mut app,
        AgentEvent::root(AgentEventPayload::WorkflowRunChanged(workflow(
            1,
            WorkflowRunState::Running,
        ))),
    ));
    assert!(handle_agent_event(
        &mut app,
        AgentEvent::root(AgentEventPayload::WorkflowRunChanged(workflow(
            3,
            WorkflowRunState::Succeeded,
        ))),
    ));
}
