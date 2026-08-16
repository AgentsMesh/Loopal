use std::sync::atomic::Ordering;

use loopal_error::TerminateReason;
use loopal_protocol::{Envelope, MessageSource};
use loopal_runtime::{LifecycleMode, workflow_input::WorkflowInputDisposition};
use loopal_test_support::chunks;

use super::workflow_input_support::{harness, harness_with_results};

const INDETERMINATE: &str = "workflow start outcome indeterminate";
const WORKFLOW_FAILURE: &str =
    "workflow input handler failed: workflow start outcome indeterminate";

#[tokio::test]
async fn handled_human_input_skips_the_ordinary_provider_turn() {
    let (mut harness, calls) = harness(vec![WorkflowInputDisposition::Handled], Vec::new()).await;
    harness
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "delegate this"))
        .await
        .unwrap();

    let output = harness.runner.run().await.unwrap();

    assert!(output.result.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(harness.recorded_messages.lock().unwrap().is_empty());
    assert!(matches!(
        harness.runner.recorded_turns()[0].outcome,
        loopal_turn::TurnOutcome::Complete
    ));
}

#[tokio::test]
async fn direct_human_input_runs_the_ordinary_provider_turn() {
    let (mut harness, calls) = harness(
        vec![WorkflowInputDisposition::Direct],
        vec![chunks::text_turn("direct answer")],
    )
    .await;
    harness
        .mailbox_tx
        .send(Envelope::new(
            MessageSource::Human,
            "main",
            "answer directly",
        ))
        .await
        .unwrap();

    let output = harness.runner.run().await.unwrap();

    assert_eq!(output.result, "direct answer");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(harness.recorded_messages.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn one_shot_workflow_handler_error_returns_real_error_without_provider_fallback() {
    let (mut harness, calls) = harness_with_results(
        LifecycleMode::Ephemeral,
        vec![Err(INDETERMINATE.into())],
        Vec::new(),
    )
    .await;
    harness
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "delegate this"))
        .await
        .unwrap();

    let output = harness.runner.run().await.unwrap();

    assert_eq!(output.terminate_reason, TerminateReason::Error);
    assert_eq!(output.result, WORKFLOW_FAILURE);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(harness.recorded_messages.lock().unwrap().is_empty());
    assert!(matches!(
        &harness.runner.recorded_turns()[0].outcome,
        loopal_turn::TurnOutcome::Error { message }
            if message == WORKFLOW_FAILURE
    ));
}

#[tokio::test]
async fn persistent_workflow_handler_error_remains_error_when_input_closes() {
    let (mut harness, calls) = harness_with_results(
        LifecycleMode::Persistent,
        vec![Err(INDETERMINATE.into())],
        Vec::new(),
    )
    .await;
    harness
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "delegate this"))
        .await
        .unwrap();
    drop(harness.mailbox_tx);
    drop(harness.control_tx);
    drop(harness.session_ctrl);

    let output = harness.runner.run().await.unwrap();

    assert_eq!(output.terminate_reason, TerminateReason::Error);
    assert_eq!(output.result, WORKFLOW_FAILURE);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(harness.recorded_messages.lock().unwrap().is_empty());
    assert!(matches!(
        &harness.runner.recorded_turns()[0].outcome,
        loopal_turn::TurnOutcome::Error { message } if message == WORKFLOW_FAILURE
    ));
}

#[tokio::test]
async fn turn_start_persist_failure_drops_input_before_workflow_or_provider_dispatch() {
    let (mut harness, calls) = harness(
        vec![WorkflowInputDisposition::Direct],
        vec![chunks::text_turn("must not run")],
    )
    .await;
    let invalid_base = harness
        .fixture
        .create_file("session-base-is-a-file", "not a directory");
    harness.runner.params.deps.session_manager =
        loopal_runtime::SessionManager::with_base_dir(invalid_base);
    harness
        .mailbox_tx
        .send(Envelope::new(
            MessageSource::Human,
            "main",
            "answer directly",
        ))
        .await
        .unwrap();

    let output = harness.runner.run().await.unwrap();

    assert!(output.result.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(harness.recorded_messages.lock().unwrap().is_empty());
    assert!(harness.runner.recorded_turns().is_empty());
}

#[tokio::test]
async fn non_human_input_bypasses_the_workflow_handler() {
    let (mut harness, calls) = harness(
        vec![WorkflowInputDisposition::Handled],
        vec![chunks::text_turn("scheduled answer")],
    )
    .await;
    harness
        .mailbox_tx
        .send(Envelope::new(
            MessageSource::Scheduled,
            "main",
            "scheduled work",
        ))
        .await
        .unwrap();

    let output = harness.runner.run().await.unwrap();

    assert_eq!(output.result, "scheduled answer");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(harness.recorded_messages.lock().unwrap().len(), 1);
}
