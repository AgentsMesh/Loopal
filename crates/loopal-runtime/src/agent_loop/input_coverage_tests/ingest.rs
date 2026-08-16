use std::sync::Arc;

use loopal_protocol::{AgentStatus, Envelope, MessageSource};

use super::super::WaitResult;
use super::support::{FAIL_ERROR, FAIL_GATE, FAIL_RUNNING, make_fixture};
use crate::SessionManager;
use crate::agent_loop::continuation_gate::GateClose;
use crate::workflow_input::{WorkflowInputDisposition, WorkflowInputHandler};

const INDETERMINATE: &str = "workflow start outcome indeterminate";
const WORKFLOW_FAILURE: &str =
    "workflow input handler failed: workflow start outcome indeterminate";

struct IndeterminateHandler;

impl WorkflowInputHandler for IndeterminateHandler {
    fn handle<'a>(
        &'a self,
        _envelope: &'a Envelope,
        _recent_context: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<WorkflowInputDisposition, String>> + Send + 'a>,
    > {
        Box::pin(std::future::ready(Err(INDETERMINATE.into())))
    }
}

#[tokio::test]
async fn workflow_handler_error_is_an_explicit_fail_closed_wait_result() {
    let mut fixture = make_fixture();
    fixture.runner.params.workflow_input_handler = Some(Arc::new(IndeterminateHandler));
    let envelope = Envelope::new(MessageSource::Human, "runtime-coverage", "delegate this");

    let result = fixture.runner.ingest_message(&envelope).await;

    assert!(matches!(
        result,
        WaitResult::WorkflowFailed(ref message) if message == WORKFLOW_FAILURE
    ));
    assert_eq!(fixture.runner.status, AgentStatus::Error);
    assert!(matches!(
        &fixture.runner.recorded_turns()[0].outcome,
        loopal_turn::TurnOutcome::Error { message } if message == WORKFLOW_FAILURE
    ));
}

#[tokio::test]
async fn suspended_automatic_input_does_not_wake_or_reopen_the_gate() {
    let mut fixture = make_fixture();
    fixture.runner.status = AgentStatus::Suspended;
    fixture
        .runner
        .continuation_gate
        .close(GateClose::UserSuspend);
    let envelope = Envelope::new(
        MessageSource::Agent("peer".into()),
        "runtime-coverage",
        "automatic",
    );

    assert!(matches!(
        fixture.runner.ingest_message(&envelope).await,
        WaitResult::MessageAdded
    ));
    assert_eq!(fixture.runner.status, AgentStatus::Suspended);
    assert!(!fixture.runner.continuation_gate.is_open());
}

#[tokio::test]
async fn failed_human_wake_keeps_the_suspension_gate_closed() {
    let mut fixture = make_fixture();
    fixture.runner.status = AgentStatus::Suspended;
    fixture
        .runner
        .continuation_gate
        .close(GateClose::UserSuspend);
    fixture.frontend.set_fail_mask(FAIL_RUNNING);

    let envelope = Envelope::new(MessageSource::Human, "runtime-coverage", "wake");
    assert!(matches!(
        fixture.runner.ingest_message(&envelope).await,
        WaitResult::MessageAdded
    ));
    assert_eq!(fixture.runner.status, AgentStatus::Suspended);
    assert!(!fixture.runner.continuation_gate.is_open());
}

#[tokio::test]
async fn workflow_failure_survives_a_failed_error_event() {
    let mut fixture = make_fixture();
    fixture.runner.params.workflow_input_handler = Some(Arc::new(IndeterminateHandler));
    fixture.frontend.set_fail_mask(FAIL_ERROR);
    let envelope = Envelope::new(MessageSource::Human, "runtime-coverage", "delegate");

    assert!(matches!(
        fixture.runner.ingest_message(&envelope).await,
        WaitResult::WorkflowFailed(ref message) if message == WORKFLOW_FAILURE
    ));
    assert_eq!(fixture.runner.status, AgentStatus::Error);
}

#[tokio::test]
async fn turn_start_and_error_event_failures_still_drop_the_envelope() {
    let mut fixture = make_fixture();
    let blocked = fixture.temp.path().join("blocked-state-root");
    std::fs::write(&blocked, "not a directory").unwrap();
    fixture.runner.params.deps.session_manager = SessionManager::with_base_dir(blocked);
    fixture.frontend.set_fail_mask(FAIL_ERROR);
    let envelope = Envelope::new(MessageSource::Human, "runtime-coverage", "drop me");

    assert!(matches!(
        fixture.runner.ingest_message(&envelope).await,
        WaitResult::MessageAdded
    ));
    assert!(fixture.runner.recorded_turns().is_empty());
    assert!(fixture.runner.pending_consumed_ids.is_empty());
}

#[tokio::test]
async fn gate_event_failure_does_not_prevent_ingest_from_reopening_the_gate() {
    let mut fixture = make_fixture();
    fixture
        .runner
        .continuation_gate
        .close(GateClose::UserSuspend);
    fixture.frontend.set_fail_mask(FAIL_GATE);
    let envelope = Envelope::new(MessageSource::Human, "runtime-coverage", "reopen");

    assert!(matches!(
        fixture.runner.ingest_message(&envelope).await,
        WaitResult::MessageAdded
    ));
    assert!(fixture.runner.continuation_gate.is_open());
    assert_eq!(fixture.runner.pending_consumed_ids.len(), 1);
}

#[tokio::test]
async fn title_persistence_failure_keeps_the_ingested_turn() {
    let mut fixture = make_fixture();
    let session_file = fixture
        .temp
        .path()
        .join("state/sessions")
        .join(&fixture.runner.params.session.id)
        .join("session.json");
    std::fs::remove_file(&session_file).unwrap();
    std::fs::create_dir(&session_file).unwrap();
    let envelope = Envelope::new(MessageSource::Human, "runtime-coverage", "durable title");

    assert!(matches!(
        fixture.runner.ingest_message(&envelope).await,
        WaitResult::MessageAdded
    ));
    assert_eq!(fixture.runner.params.session.title, "durable title");
    assert_eq!(fixture.runner.recorded_turns().len(), 1);
}
