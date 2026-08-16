use std::sync::Arc;

use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, SubAgentSpawn, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode, PanelKind};
use loopal_tui::input::InputAction;
use loopal_tui::key_dispatch_for_test::dispatch;
use tokio::sync::{mpsc, watch};

fn make_app() -> (App, watch::Receiver<u64>) {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(8);
    let (permission_tx, _) = mpsc::channel::<bool>(8);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(8);
    let (interrupt_tx, interrupt_rx) = watch::channel(0_u64);
    let session = SessionController::new(
        control_tx,
        permission_tx,
        question_tx,
        Default::default(),
        Arc::new(interrupt_tx),
    );
    (App::new(session, std::env::temp_dir()), interrupt_rx)
}

fn spawn(app: &mut App, name: &str) {
    app.dispatch_event(AgentEvent::named(
        name,
        AgentEventPayload::SubAgentSpawned(SubAgentSpawn {
            name: name.into(),
            agent_id: format!("id-{name}"),
            parent: Some("main".into()),
            model: None,
            session_id: None,
        }),
    ));
    app.dispatch_event(AgentEvent::named(name, AgentEventPayload::Started));
}

#[tokio::test]
async fn terminate_is_a_noop_without_focus_or_when_root_is_focused() {
    let (mut app, interrupt_rx) = make_app();
    dispatch(&mut app, InputAction::TerminateFocusedAgent).await;
    assert!(!interrupt_rx.has_changed().unwrap());

    app.section_mut(PanelKind::Agents).focused = Some("main".into());
    dispatch(&mut app, InputAction::TerminateFocusedAgent).await;
    assert!(!interrupt_rx.has_changed().unwrap());
    assert_eq!(
        app.section(PanelKind::Agents).focused.as_deref(),
        Some("main")
    );
}

#[tokio::test]
async fn terminating_a_background_child_keeps_the_current_view() {
    let (mut app, mut interrupt_rx) = make_app();
    spawn(&mut app, "worker");
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());

    dispatch(&mut app, InputAction::TerminateFocusedAgent).await;

    interrupt_rx.changed().await.unwrap();
    assert_eq!(*interrupt_rx.borrow_and_update(), 1);
    assert_eq!(app.session.lock().active_view, "main");
    assert_eq!(app.section(PanelKind::Agents).focused, None);
}

#[tokio::test]
async fn terminating_the_open_child_returns_to_root_and_resets_scroll() {
    let (mut app, mut interrupt_rx) = make_app();
    spawn(&mut app, "worker");
    assert!(app.session.enter_agent_view("worker"));
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.content_scroll.offset = 9;

    dispatch(&mut app, InputAction::TerminateFocusedAgent).await;

    interrupt_rx.changed().await.unwrap();
    assert_eq!(app.session.lock().active_view, "main");
    assert_eq!(app.content_scroll.offset, 0);
}

#[tokio::test]
async fn terminating_the_last_non_live_row_falls_back_to_input() {
    let (mut app, mut interrupt_rx) = make_app();
    spawn(&mut app, "worker");
    app.dispatch_event(AgentEvent::named("worker", AgentEventPayload::Finished));
    app.section_mut(PanelKind::Agents).focused = Some("worker".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Agents);

    dispatch(&mut app, InputAction::TerminateFocusedAgent).await;

    interrupt_rx.changed().await.unwrap();
    assert_eq!(app.focus_mode, FocusMode::Input);
    assert_eq!(app.section(PanelKind::Agents).focused, None);
}
