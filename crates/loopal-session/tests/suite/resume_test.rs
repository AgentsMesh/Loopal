//! Tests for session resume: event handling and controller resume_session method.

use std::sync::Arc;

use loopal_protocol::UserQuestionResponse;
use loopal_protocol::{AgentEvent, AgentEventPayload, ControlCommand};
use loopal_session::SessionController;
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;
use tokio::sync::mpsc;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

fn make_controller() -> (SessionController, mpsc::Receiver<ControlCommand>) {
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let ctrl = SessionController::new(
        "test-model".to_string(),
        "act".to_string(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    (ctrl, control_rx)
}

// ---------------------------------------------------------------------------
// apply_event: SessionResumed updates root_session_id
// ---------------------------------------------------------------------------

#[test]
fn test_session_resumed_event_updates_root_session_id() {
    let mut state = make_state();
    state.root_session_id = Some("old-session".to_string());

    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::SessionResumed {
            session_id: "new-session-xyz".into(),
            message_count: 10,
        }),
    );

    assert_eq!(state.root_session_id.as_deref(), Some("new-session-xyz"),);
}

#[test]
fn test_session_resumed_event_sets_root_session_id_from_none() {
    let mut state = make_state();
    assert!(state.root_session_id.is_none());

    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::SessionResumed {
            session_id: "first-session".into(),
            message_count: 0,
        }),
    );

    assert_eq!(state.root_session_id.as_deref(), Some("first-session"),);
}

// ---------------------------------------------------------------------------
// SessionController::resume_session — clears state + sends command
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_controller_resume_session_clears_display() {
    let (ctrl, _rx) = make_controller();

    // Populate some display state
    ctrl.handle_event(AgentEvent::root(AgentEventPayload::Stream {
        text: "hello world".into(),
    }));
    {
        let state = ctrl.lock();
        assert!(!state.agents["main"].conversation.streaming_text.is_empty());
    }

    ctrl.resume_session("target-session").await;

    let state = ctrl.lock();
    let conv = &state.agents["main"].conversation;
    assert!(conv.messages.is_empty(), "messages should be cleared");
    assert!(
        conv.streaming_text.is_empty(),
        "streaming should be cleared"
    );
    assert_eq!(conv.turn_count, 0);
    assert_eq!(conv.input_tokens, 0);
    assert_eq!(conv.output_tokens, 0);
}

#[tokio::test]
async fn test_controller_resume_session_updates_root_id() {
    let (ctrl, _rx) = make_controller();
    ctrl.set_root_session_id("old-id");

    ctrl.resume_session("new-id-abc").await;

    let state = ctrl.lock();
    assert_eq!(state.root_session_id.as_deref(), Some("new-id-abc"));
}

#[tokio::test]
async fn test_controller_resume_session_sends_control_command() {
    let (ctrl, mut rx) = make_controller();

    ctrl.resume_session("target-session").await;

    let cmd = rx.try_recv().expect("should receive a control command");
    if let ControlCommand::ResumeSession(sid) = cmd {
        assert_eq!(sid, "target-session");
    } else {
        panic!("expected ControlCommand::ResumeSession, got {cmd:?}");
    }
}
