use std::sync::Arc;
use std::sync::atomic::Ordering;

use loopal_protocol::{
    QualifiedAddress, WorkflowCancelRequest, WorkflowFailureClass, WorkflowRequestId,
    WorkflowRunId, WorkflowRunSnapshot, WorkflowRunState, WorkflowTerminalDeliveryId,
    WorkflowTerminalDisposition,
};

use super::super::WorkflowCoordinatorError;
use super::support::{owner, spec};
use super::terminal_delivery_support::{
    TestTerminalSink, coordinator, coordinator_with_seed, coordinator_without_intent,
};

#[tokio::test]
async fn recovered_terminal_waits_for_activation_then_acks_applied_once() {
    let workflow_owner = owner("session", "root");
    let run = terminal_run("wrun_applied", WorkflowRunState::Succeeded, 7);
    let sink = Arc::new(TestTerminalSink::new([Ok(
        WorkflowTerminalDisposition::Applied,
    )]));
    let (handle, task, journal) = coordinator(run, [], sink.clone());

    handle.recover(workflow_owner.clone()).await.unwrap();
    tokio::task::yield_now().await;
    assert!(sink.deliveries().is_empty());
    handle
        .activate_terminal_deliveries(workflow_owner.clone())
        .await
        .unwrap();
    sink.wait_for_deliveries(1).await;
    journal.wait_for_delivery_acks(1).await;
    handle.tick(20).await.unwrap();
    tokio::task::yield_now().await;

    assert_eq!(sink.deliveries().len(), 1);
    assert_eq!(journal.delivery_acks().len(), 1);
    assert_eq!(journal.delivery_acks()[0].0, workflow_owner);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn queued_delivery_retries_same_identity_and_only_applied_is_acked() {
    let workflow_owner = owner("session", "root");
    let run = terminal_run("wrun_queued", WorkflowRunState::Failed, 9);
    let sink = Arc::new(TestTerminalSink::new([
        Ok(WorkflowTerminalDisposition::Retryable {
            reason: "root turn persistence is temporarily unavailable".into(),
        }),
        Ok(WorkflowTerminalDisposition::AlreadyApplied),
    ]));
    let (handle, task, journal) = coordinator(run, [], sink.clone());
    handle.recover(workflow_owner.clone()).await.unwrap();
    handle
        .activate_terminal_deliveries(workflow_owner)
        .await
        .unwrap();
    sink.wait_for_deliveries(1).await;
    assert!(journal.delivery_acks().is_empty());

    handle.tick(20).await.unwrap();
    sink.wait_for_deliveries(2).await;
    journal.wait_for_delivery_acks(1).await;
    let deliveries = sink.deliveries();
    assert_eq!(deliveries[0].delivery_id, deliveries[1].delivery_id);
    assert_eq!(journal.delivery_acks().len(), 1);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn queued_retry_reuses_the_original_redacted_payload() {
    let run = terminal_run("wrun_stable_payload", WorkflowRunState::Failed, 10);
    let sink = Arc::new(TestTerminalSink::new([
        Ok(WorkflowTerminalDisposition::Queued),
        Ok(WorkflowTerminalDisposition::AlreadyApplied),
    ]));
    let seed = loopal_output_guard::FinalSinkRedactionSeed::new();
    let (handle, task, journal) = coordinator_with_seed(run, [], sink.clone(), seed.clone());
    let workflow_owner = owner("session", "root");
    handle.recover(workflow_owner.clone()).await.unwrap();
    handle
        .activate_terminal_deliveries(workflow_owner)
        .await
        .unwrap();
    sink.wait_for_deliveries(1).await;
    seed.observe("late", "final failure".into()).unwrap();

    handle.tick(20).await.unwrap();
    sink.wait_for_deliveries(2).await;
    journal.wait_for_delivery_acks(1).await;
    let deliveries = sink.deliveries();
    assert_eq!(deliveries[0], deliveries[1]);
    assert!(deliveries[1].content.contains("final failure"));
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn recovered_ack_suppresses_delivery_and_ack_append_failure_is_fatal() {
    let workflow_owner = owner("session", "root");
    let run = terminal_run("wrun_recovered_ack", WorkflowRunState::Cancelled, 11);
    let id = WorkflowTerminalDeliveryId::new("session", run.id.clone(), run.revision);
    let sink = Arc::new(TestTerminalSink::new([]));
    let (handle, task, journal) = coordinator(run, [id], sink.clone());
    handle.recover(workflow_owner.clone()).await.unwrap();
    handle
        .activate_terminal_deliveries(workflow_owner)
        .await
        .unwrap();
    handle.tick(20).await.unwrap();
    assert!(sink.deliveries().is_empty());
    assert!(journal.delivery_acks().is_empty());
    drop(handle);
    task.await.unwrap();

    let run = terminal_run("wrun_ack_failure", WorkflowRunState::Succeeded, 12);
    let sink = Arc::new(TestTerminalSink::new([Ok(
        WorkflowTerminalDisposition::Applied,
    )]));
    let (handle, task, journal) = coordinator(run, [], sink);
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    let workflow_owner = owner("session", "root");
    handle.recover(workflow_owner.clone()).await.unwrap();
    handle
        .activate_terminal_deliveries(workflow_owner)
        .await
        .unwrap();
    journal.wait_for_delivery_ack_attempt().await;
    assert_eq!(
        handle.tick(20).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn terminal_sink_panic_poisoning_is_observable_on_tick() {
    let run = terminal_run("wrun_sink_panic", WorkflowRunState::Succeeded, 13);
    let sink = Arc::new(TestTerminalSink::new([]));
    sink.push_panic();
    let (handle, task, journal) = coordinator(run, [], sink.clone());
    let workflow_owner = owner("session", "root");
    handle.recover(workflow_owner.clone()).await.unwrap();
    handle
        .activate_terminal_deliveries(workflow_owner)
        .await
        .unwrap();
    sink.wait_for_deliveries(1).await;

    let mut observed = None;
    for _ in 0..100 {
        match handle.tick(20).await {
            Err(error) => {
                observed = Some(error);
                break;
            }
            Ok(()) => tokio::task::yield_now().await,
        }
    }
    assert_eq!(observed, Some(WorkflowCoordinatorError::Unavailable));
    assert!(journal.delivery_acks().is_empty());
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn poisoned_owner_rejects_an_in_flight_terminal_ack() {
    let run = terminal_run("wrun_poisoned_ack", WorkflowRunState::Succeeded, 14);
    let sink = Arc::new(TestTerminalSink::new([Ok(
        WorkflowTerminalDisposition::Applied,
    )]));
    let gate = sink.block_next_delivery();
    let (handle, task, journal) = coordinator(run, [], sink);
    let workflow_owner = owner("session", "root");

    handle.recover(workflow_owner.clone()).await.unwrap();
    handle
        .activate_terminal_deliveries(workflow_owner.clone())
        .await
        .unwrap();
    gate.wait_started().await;

    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    assert_eq!(
        handle
            .cancel(
                workflow_owner.clone(),
                WorkflowCancelRequest {
                    request_id: WorkflowRequestId::new("wreq_poisoned_ack_cancel"),
                    run_id: WorkflowRunId::new("wrun_poisoned_ack"),
                    reason: Some("poison before terminal acknowledgement".into()),
                },
            )
            .await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );

    gate.release();
    for _ in 0..100 {
        handle.tick(20).await.unwrap();
        if journal.delivery_ack_attempts.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(
        journal.delivery_ack_attempts.load(Ordering::SeqCst),
        0,
        "a poisoned owner must not append a terminal delivery acknowledgement"
    );
    assert!(journal.delivery_acks().is_empty());

    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn poisoned_owner_rejects_terminal_delivery_activation() {
    let run = terminal_run("wrun_poisoned_activation", WorkflowRunState::Succeeded, 15);
    let sink = Arc::new(TestTerminalSink::new([]));
    let (handle, task, journal) = coordinator_without_intent(run, [], sink);
    let workflow_owner = owner("session", "root");

    handle.recover(workflow_owner.clone()).await.unwrap();
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    assert_eq!(
        handle
            .cancel(
                workflow_owner.clone(),
                WorkflowCancelRequest {
                    request_id: WorkflowRequestId::new("wreq_poisoned_activation_cancel"),
                    run_id: WorkflowRunId::new("wrun_poisoned_activation"),
                    reason: Some("poison before terminal activation".into()),
                },
            )
            .await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );

    assert_eq!(
        handle.activate_terminal_deliveries(workflow_owner).await,
        Err(WorkflowCoordinatorError::OwnerPoisoned),
        "a poisoned owner must not recreate terminal delivery intents"
    );
    assert!(journal.delivery_intents().is_empty());
    assert!(journal.delivery_acks().is_empty());
    drop(handle);
    task.await.unwrap();
}

fn terminal_run(id: &str, state: WorkflowRunState, revision: u64) -> WorkflowRunSnapshot {
    let mut run = WorkflowRunSnapshot::planned(
        WorkflowRunId::new(id),
        QualifiedAddress::local("root"),
        spec(),
        1,
    );
    run.state = state;
    run.revision = revision;
    match state {
        WorkflowRunState::Succeeded => {
            run.result = Some(loopal_protocol::WorkflowOutput::Text("final result".into()));
        }
        WorkflowRunState::Failed => {
            run.failure = Some(loopal_protocol::WorkflowAttemptFailure {
                class: WorkflowFailureClass::Permanent,
                reason: "final failure".into(),
            });
        }
        WorkflowRunState::Cancelled => {}
        _ => unreachable!(),
    }
    run
}
