use std::sync::atomic::Ordering;

use loopal_protocol::{Envelope, MessageSource};
use loopal_runtime::workflow_input::WorkflowInputDisposition;
use loopal_test_support::chunks;

use super::workflow_input_support::harness;

#[tokio::test]
async fn handled_then_direct_preserves_the_direct_provider_turn() {
    let (mut harness, calls) = harness(
        vec![
            WorkflowInputDisposition::Handled,
            WorkflowInputDisposition::Direct,
        ],
        vec![chunks::text_turn("direct answer")],
    )
    .await;
    send(&harness.mailbox_tx, MessageSource::Human, "delegate first").await;
    send(&harness.mailbox_tx, MessageSource::Human, "answer second").await;

    let output = harness.runner.run().await.unwrap();

    assert_eq!(output.result, "direct answer");
    assert_sequence(&harness, &calls, 2, 1);
}

#[tokio::test]
async fn direct_then_handled_does_not_cancel_the_completed_direct_turn() {
    let (mut harness, calls) = harness(
        vec![
            WorkflowInputDisposition::Direct,
            WorkflowInputDisposition::Handled,
        ],
        vec![chunks::text_turn("direct answer")],
    )
    .await;
    send(&harness.mailbox_tx, MessageSource::Human, "answer first").await;
    send(&harness.mailbox_tx, MessageSource::Human, "delegate second").await;

    let output = harness.runner.run().await.unwrap();

    assert_eq!(output.result, "direct answer");
    assert_sequence(&harness, &calls, 2, 1);
}

#[tokio::test]
async fn handled_human_then_scheduled_still_runs_the_scheduled_turn() {
    let (mut harness, calls) = harness(
        vec![WorkflowInputDisposition::Handled],
        vec![chunks::text_turn("scheduled answer")],
    )
    .await;
    send(&harness.mailbox_tx, MessageSource::Human, "delegate first").await;
    send(
        &harness.mailbox_tx,
        MessageSource::Scheduled,
        "scheduled second",
    )
    .await;

    let output = harness.runner.run().await.unwrap();

    assert_eq!(output.result, "scheduled answer");
    assert_sequence(&harness, &calls, 1, 1);
}

#[tokio::test]
async fn multiple_direct_envelopes_each_run_their_own_provider_turn() {
    let (mut harness, calls) = harness(
        vec![
            WorkflowInputDisposition::Direct,
            WorkflowInputDisposition::Direct,
        ],
        vec![
            chunks::text_turn("first answer"),
            chunks::text_turn("second answer"),
        ],
    )
    .await;
    send(&harness.mailbox_tx, MessageSource::Human, "first input").await;
    send(&harness.mailbox_tx, MessageSource::Human, "second input").await;

    let output = harness.runner.run().await.unwrap();

    assert_eq!(output.result, "second answer");
    assert_sequence(&harness, &calls, 2, 2);
}

async fn send(mailbox: &tokio::sync::mpsc::Sender<Envelope>, source: MessageSource, text: &str) {
    mailbox
        .send(Envelope::new(source, "main", text))
        .await
        .unwrap();
}

fn assert_sequence(
    harness: &loopal_test_support::IntegrationHarness,
    calls: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
    handler_calls: usize,
    provider_calls: usize,
) {
    assert_eq!(calls.load(Ordering::SeqCst), handler_calls);
    assert_eq!(
        harness.recorded_messages.lock().unwrap().len(),
        provider_calls
    );
    assert_eq!(harness.runner.recorded_turns().len(), 2);
    assert!(
        harness
            .runner
            .recorded_turns()
            .iter()
            .all(|turn| matches!(turn.outcome, loopal_turn::TurnOutcome::Complete))
    );
}
