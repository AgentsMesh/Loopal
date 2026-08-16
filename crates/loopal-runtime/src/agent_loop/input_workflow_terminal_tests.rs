use std::path::Path;
use std::sync::Arc;

use loopal_config::Settings;
use loopal_context::ContextBudget;
use loopal_kernel::Kernel;
use loopal_protocol::{
    AgentStatus, WorkflowRunId, WorkflowRunState, WorkflowTerminalDeliveryId,
    WorkflowTerminalDisposition, WorkflowTerminalNotification, WorkflowTerminalOutcome,
};

use super::*;
use crate::agent_input::{AgentInput, WorkflowTerminalRequest};
use crate::frontend::{DenyAllHandler, UnifiedFrontend, UnsupportedQuestionHandler};
use crate::{AgentConfig, AgentDeps, AgentLoopParamsBuilder, InterruptHandle, SessionManager};

pub(super) fn notification(session_id: &str) -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(
            session_id,
            WorkflowRunId::new("wrun_terminal_test"),
            3,
        ),
        state: WorkflowRunState::Succeeded,
        run_goal: "finish durable work".into(),
        outcome: WorkflowTerminalOutcome::Succeeded {
            result: "complete".into(),
        },
        content: "workflow completed".into(),
    }
}

pub(super) fn runner(base: &Path, session_id: &str) -> AgentLoopRunner {
    let session_manager = SessionManager::with_base_dir(base.to_path_buf());
    let session = session_manager
        .create_session_with_id(base, "test-model", session_id)
        .unwrap();
    let (event_tx, _event_rx) = tokio::sync::mpsc::channel(8);
    let (_message_tx, message_rx) = tokio::sync::mpsc::channel(8);
    let (_control_tx, control_rx) = tokio::sync::mpsc::channel(8);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        message_rx,
        control_rx,
        None,
        Box::new(DenyAllHandler),
        Box::new(UnsupportedQuestionHandler),
    ));
    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        AgentDeps {
            kernel: Arc::new(Kernel::new(Settings::default()).unwrap()),
            frontend,
            session_manager,
            decision_context: crate::frontend::DecisionContext::with_cwd(
                base.to_string_lossy().into_owned(),
            ),
            protected_effect_audit: Arc::new(loopal_tool_api::NoopProtectedEffectAudit),
        },
        session,
        ContextBudget::calculate(200_000, "", 0, 16_000),
        InterruptHandle::new(),
    )
    .build();
    AgentLoopRunner::new(params)
}

pub(super) fn turns_file(base: &Path, session_id: &str) -> std::path::PathBuf {
    base.join("sessions").join(session_id).join("turns.jsonl")
}

pub(super) async fn apply(
    runner: &mut AgentLoopRunner,
    notification: WorkflowTerminalNotification,
) -> (bool, WorkflowTerminalDisposition) {
    let (request, receiver) = WorkflowTerminalRequest::tracked(notification);
    let execute = runner.apply_workflow_terminal(request).await;
    let disposition = receiver.borrow().clone().expect("runtime acknowledgement");
    (execute, disposition)
}

#[tokio::test]
async fn new_delivery_is_durable_and_duplicate_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let mut runner = runner(temp.path(), "session-terminal");
    let notification = notification("session-terminal");

    assert_eq!(
        apply(&mut runner, notification.clone()).await,
        (true, WorkflowTerminalDisposition::Applied)
    );
    let events = runner
        .params
        .deps
        .session_manager
        .load_turn_events("session-terminal")
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        apply(&mut runner, notification).await,
        (false, WorkflowTerminalDisposition::AlreadyApplied)
    );
    assert_eq!(
        runner
            .params
            .deps
            .session_manager
            .load_turn_events("session-terminal")
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn same_delivery_id_with_different_payload_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let mut runner = runner(temp.path(), "session-conflict");
    let notification = notification("session-conflict");
    assert!(apply(&mut runner, notification.clone()).await.0);

    let mut conflict = notification;
    conflict.content = "different terminal content".into();
    let (execute, disposition) = apply(&mut runner, conflict).await;
    assert!(!execute);
    assert!(matches!(
        disposition,
        WorkflowTerminalDisposition::Rejected { reason } if reason.contains("conflicts")
    ));
}

#[tokio::test]
async fn wrong_session_is_rejected_and_persisted_delivery_recovers() {
    let temp = tempfile::tempdir().unwrap();
    let mut first = runner(temp.path(), "session-recovery");
    let terminal = notification("session-recovery");
    assert!(apply(&mut first, terminal.clone()).await.0);
    drop(first);

    let mut recovered = runner(temp.path(), "session-recovery");
    let persisted_turns = recovered
        .params
        .deps
        .session_manager
        .load_turns("session-recovery")
        .unwrap();
    recovered
        .turns
        .replace_store(loopal_context::TurnStore::from_turns(persisted_turns));
    assert_eq!(
        apply(&mut recovered, terminal).await,
        (true, WorkflowTerminalDisposition::AlreadyApplied)
    );
    assert!(recovered.ensure_resume_turn_record().await.unwrap());
    assert_eq!(recovered.recorded_turns().len(), 2);
    assert!(matches!(
        recovered.recorded_turns().last().map(|turn| &turn.trigger),
        Some(loopal_turn::TurnTrigger::WorkflowResult { .. })
    ));
    recovered.end_turn_record(loopal_turn::TurnOutcome::Complete);
    assert_eq!(
        apply(&mut recovered, notification("session-recovery")).await,
        (false, WorkflowTerminalDisposition::AlreadyApplied)
    );

    let (_, disposition) = apply(&mut recovered, notification("other-session")).await;
    assert!(matches!(
        disposition,
        WorkflowTerminalDisposition::Rejected { reason } if reason.contains("different session")
    ));
}

#[tokio::test]
async fn completed_persisted_delivery_is_not_reexecuted() {
    let temp = tempfile::tempdir().unwrap();
    let mut first = runner(temp.path(), "session-completed");
    let terminal = notification("session-completed");
    assert!(apply(&mut first, terminal.clone()).await.0);
    first.end_turn_record(loopal_turn::TurnOutcome::Complete);
    drop(first);

    let mut recovered = runner(temp.path(), "session-completed");
    assert_eq!(
        apply(&mut recovered, terminal).await,
        (false, WorkflowTerminalDisposition::AlreadyApplied)
    );
}

#[test]
fn suspended_session_defers_workflow_terminal_input() {
    let temp = tempfile::tempdir().unwrap();
    let mut runner = runner(temp.path(), "session-suspended");
    runner.status = AgentStatus::Suspended;
    let (request, _receiver) = WorkflowTerminalRequest::tracked(notification("session-suspended"));
    assert!(runner.should_defer_frontend_input(&AgentInput::WorkflowTerminal(request)));
}
