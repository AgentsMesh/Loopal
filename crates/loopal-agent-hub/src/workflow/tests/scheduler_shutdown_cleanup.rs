use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{
    QualifiedAddress, WorkflowAttemptId, WorkflowCancelRequest, WorkflowNodeId, WorkflowRequestId,
    WorkflowRunId,
};
use tokio::sync::{mpsc, oneshot};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, prepared_worker, test_spawner};
use super::support::{owner, request};
use crate::workflow::WorkflowOwner;
use crate::workflow::command::WorkflowCommand;
use crate::workflow::scheduler::{AttemptKey, WorkflowCleanupStatus, WorkflowPreparedDelivery};

#[tokio::test]
async fn coordinator_shutdown_aborts_external_and_local_preparation() {
    let run_id = WorkflowRunId::new("wrun_prepare_shutdown");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        [100, 101, 102, 103, 104, 105],
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_prepare_shutdown")],
        journal,
        spawner,
    );
    let owner = owner("session-prepare-shutdown", "root");
    let mut start = request("wreq_prepare_shutdown");
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();
    handle.start(owner.clone(), start).await.unwrap();
    handle.schedule(owner, run_id).await.unwrap();
    let SpawnerEffect::Prepare {
        response: prepare, ..
    } = control.next().await
    else {
        panic!("expected preparation")
    };

    let shutdown_handle = handle.clone();
    let shutdown = tokio::spawn(async move { shutdown_handle.shutdown().await });
    let SpawnerEffect::AbortPrepare {
        response: abort, ..
    } = control.next().await
    else {
        panic!("expected preparation abort during coordinator drain")
    };
    assert!(abort.send(WorkflowCleanupStatus::Confirmed).is_ok());
    shutdown.await.unwrap().unwrap();
    task.await.unwrap();
    assert!(prepare.is_closed());
    control.assert_drained().await;
}

#[tokio::test]
async fn coordinator_shutdown_awaits_existing_late_preparation_shutdown() {
    let run_id = WorkflowRunId::new("wrun_late_prepare_shutdown");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        [100, 101, 102, 103, 104, 105, 106],
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_late_prepare_shutdown")],
        journal,
        spawner,
    );
    let owner = owner("session-late-prepare-shutdown", "root");
    let mut start = request("wreq_late_prepare_shutdown");
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();
    handle.start(owner.clone(), start).await.unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();
    let SpawnerEffect::Prepare {
        response: prepare, ..
    } = control.next().await
    else {
        panic!("expected preparation")
    };
    handle
        .cancel(
            owner,
            WorkflowCancelRequest {
                request_id: WorkflowRequestId::new("wreq_late_prepare_shutdown_cancel"),
                run_id,
                reason: Some("stop before preparation completes".into()),
            },
        )
        .await
        .unwrap();
    let SpawnerEffect::AbortPrepare {
        response: abort, ..
    } = control.next().await
    else {
        panic!("expected preparation abort")
    };

    let (worker, outcome) = prepared_worker("late-shutdown-worker", 43);
    assert!(prepare.send(Ok(worker)).is_ok());
    let SpawnerEffect::Shutdown {
        execution,
        response: late_shutdown,
    } = control.next().await
    else {
        panic!("expected late preparation shutdown")
    };
    assert_eq!(execution.address.agent, "late-shutdown-worker");
    assert_eq!(execution.connection_generation, 43);

    let shutdown_handle = handle.clone();
    let shutdown = tokio::spawn(async move { shutdown_handle.shutdown().await });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), control.next())
            .await
            .is_err(),
        "coordinator drain started a second late preparation shutdown"
    );
    assert!(!shutdown.is_finished());

    assert!(late_shutdown.send(WorkflowCleanupStatus::Confirmed).is_ok());
    shutdown.await.unwrap().unwrap();
    task.await.unwrap();
    assert!(outcome.is_closed());
    control.assert_idle().await;
    let _ = abort.send(WorkflowCleanupStatus::Confirmed);
}

#[tokio::test]
async fn closing_a_full_callback_queue_contains_the_queued_prepared_lease() {
    let (spawner, control) = test_spawner();
    let (worker, outcome) = prepared_worker("queued-drop-worker", 41);
    let delivery = WorkflowPreparedDelivery::new(Ok(worker), spawner);
    let (commands, mut receiver) = mpsc::channel(1);
    let (shutdown, _) = oneshot::channel();
    commands
        .send(WorkflowCommand::Shutdown { response: shutdown })
        .await
        .unwrap();

    let owner = WorkflowOwner::new("session-queue-drop", QualifiedAddress::local("root"));
    let key = AttemptKey {
        run_id: WorkflowRunId::new("wrun_queue_drop"),
        node_id: WorkflowNodeId::new("wnode_queue_drop"),
        attempt_id: WorkflowAttemptId::new("watt_queue_drop"),
    };
    let queued = tokio::spawn(async move {
        let _ = commands
            .send(WorkflowCommand::WorkerPrepared {
                owner,
                key,
                prepared: delivery,
            })
            .await;
    });
    tokio::task::yield_now().await;
    assert!(
        !queued.is_finished(),
        "prepared callback must be backpressured"
    );

    receiver.close();
    drop(receiver);
    queued.await.unwrap();
    let SpawnerEffect::Shutdown {
        execution,
        response,
        ..
    } = control.next().await
    else {
        panic!("expected queued prepared lease containment")
    };
    assert_eq!(execution.address.agent, "queued-drop-worker");
    assert_eq!(execution.connection_generation, 41);
    assert!(response.send(WorkflowCleanupStatus::Confirmed).is_ok());
    assert!(outcome.is_closed());
}
