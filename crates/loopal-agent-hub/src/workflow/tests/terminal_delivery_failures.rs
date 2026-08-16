use std::sync::Arc;

use loopal_protocol::{
    QualifiedAddress, WorkflowFailureClass, WorkflowRunId, WorkflowRunSnapshot, WorkflowRunState,
    WorkflowTerminalDeliveryId, WorkflowTerminalDisposition, WorkflowTerminalNotification,
    WorkflowTerminalOutcome,
};
use tokio::sync::{Mutex, mpsc};

use super::super::WorkflowCoordinatorError;
use super::super::terminal_delivery::{HubWorkflowTerminalSink, WorkflowTerminalSink};
use super::support::{owner, spec};
use super::terminal_delivery_support::{TestTerminalSink, coordinator};

#[tokio::test]
async fn permanent_rejection_poisons_owner_without_ack() {
    let run = terminal_run("wrun_rejected", 10);
    let sink = Arc::new(TestTerminalSink::new([Ok(
        WorkflowTerminalDisposition::Rejected {
            reason: "payload conflict".into(),
        },
    )]));
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
    assert_eq!(observed, Some(WorkflowCoordinatorError::RecoveryConflict));
    assert!(journal.delivery_acks().is_empty());
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn hub_terminal_sink_rejects_invalid_notification_before_ipc_lookup() {
    let (events, _receiver) = mpsc::channel(1);
    let hub = Arc::new(Mutex::new(crate::Hub::new(events)));
    let sink = HubWorkflowTerminalSink::new(hub);
    let notification = WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new("session", WorkflowRunId::new("run"), 0),
        state: WorkflowRunState::Failed,
        run_goal: "goal".into(),
        outcome: WorkflowTerminalOutcome::Failed {
            class: WorkflowFailureClass::Permanent,
            reason: "failure".into(),
        },
        content: "failure".into(),
    };

    let error = sink.deliver(&owner("session", "root"), notification).await;
    assert_eq!(
        error,
        Err("invalid workflow terminal notification: InvalidTerminalRevision".into())
    );
}

#[tokio::test]
async fn hub_terminal_sink_requires_the_managed_root_connection() {
    let (events, _receiver) = mpsc::channel(1);
    let hub = Arc::new(Mutex::new(crate::Hub::new(events)));
    let sink = HubWorkflowTerminalSink::new(hub);
    let notification = WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new("session", WorkflowRunId::new("run"), 1),
        state: WorkflowRunState::Failed,
        run_goal: "goal".into(),
        outcome: WorkflowTerminalOutcome::Failed {
            class: WorkflowFailureClass::Permanent,
            reason: "failure".into(),
        },
        content: "failure".into(),
    };

    let error = sink.deliver(&owner("session", "root"), notification).await;
    assert_eq!(error, Err("workflow root Agent is not connected".into()));
}

fn terminal_run(id: &str, revision: u64) -> WorkflowRunSnapshot {
    let mut run = WorkflowRunSnapshot::planned(
        WorkflowRunId::new(id),
        QualifiedAddress::local("root"),
        spec(),
        1,
    );
    run.state = WorkflowRunState::Failed;
    run.revision = revision;
    run.failure = Some(loopal_protocol::WorkflowAttemptFailure {
        class: WorkflowFailureClass::Permanent,
        reason: "final failure".into(),
    });
    run
}
