use loopal_protocol::{
    QualifiedAddress, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId,
};
use tokio::sync::oneshot;

use super::super::WorkflowOwner;
use super::super::scheduler::{
    ActiveAttempt, ActiveAttemptPhase, AttemptKey, PendingAttempt, WorkflowCleanupStatus,
    WorkflowWorkerOutcome,
};
use crate::types::AgentExecutionRef;

fn identity() -> (WorkflowOwner, AttemptKey, AgentExecutionRef) {
    (
        WorkflowOwner::new("session-cleanup-ownership", QualifiedAddress::local("root")),
        AttemptKey {
            run_id: WorkflowRunId::new("wrun_cleanup_ownership"),
            node_id: WorkflowNodeId::new("wnode_cleanup_ownership"),
            attempt_id: WorkflowAttemptId::new("watt_cleanup_ownership"),
        },
        AgentExecutionRef::local("worker-cleanup-ownership", 7),
    )
}

#[tokio::test]
async fn dropping_active_record_detaches_shutdown_supervisor() {
    let (started, started_rx) = oneshot::channel();
    let (release, release_rx) = oneshot::channel();
    let (finished, finished_rx) = oneshot::channel();
    let waiter = tokio::spawn(async move {
        started.send(()).unwrap();
        release_rx.await.unwrap();
        finished.send(()).unwrap();
        WorkflowCleanupStatus::Confirmed
    });
    started_rx.await.unwrap();

    let (owner, key, execution) = identity();
    let (_outcome, outcome_rx) = oneshot::channel::<WorkflowWorkerOutcome>();
    let active = ActiveAttempt {
        owner,
        key,
        execution,
        outcome: Some(outcome_rx),
        outcome_waiter: None,
        shutdown_waiter: Some(waiter),
        deadline_unix_ms: 1,
        shutdown_after_unix_ms: Some(2),
        phase: ActiveAttemptPhase::ShuttingDown,
        stop: None,
    };
    drop(active);

    release.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), finished_rx)
        .await
        .expect("shutdown supervisor was aborted with the scheduler record")
        .unwrap();
}

#[tokio::test]
async fn dropping_pending_record_detaches_abort_supervisor() {
    let (started, started_rx) = oneshot::channel();
    let (release, release_rx) = oneshot::channel();
    let (finished, finished_rx) = oneshot::channel();
    let waiter = tokio::spawn(async move {
        started.send(()).unwrap();
        release_rx.await.unwrap();
        finished.send(()).unwrap();
        WorkflowCleanupStatus::Confirmed
    });
    started_rx.await.unwrap();

    let (owner, key, _execution) = identity();
    let pending = PendingAttempt {
        owner,
        key: key.clone(),
        causation: WorkflowPermissionCausation {
            run_id: key.run_id,
            node_id: key.node_id,
            attempt_id: key.attempt_id,
        },
        deadline_unix_ms: 1,
        prepare_abort: None,
        abort_waiter: Some(waiter),
        abort_requested: true,
        abort_status: None,
        delivery_finished: false,
        late_execution: None,
        late_shutdown_waiter: None,
        stop: None,
    };
    drop(pending);

    release.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), finished_rx)
        .await
        .expect("abort supervisor was aborted with the pending record")
        .unwrap();
}

#[tokio::test]
async fn dropping_pending_record_detaches_late_shutdown_supervisor() {
    let (started, started_rx) = oneshot::channel();
    let (release, release_rx) = oneshot::channel();
    let (finished, finished_rx) = oneshot::channel();
    let waiter = tokio::spawn(async move {
        started.send(()).unwrap();
        release_rx.await.unwrap();
        finished.send(()).unwrap();
        WorkflowCleanupStatus::Confirmed
    });
    started_rx.await.unwrap();

    let (owner, key, execution) = identity();
    let pending = PendingAttempt {
        owner,
        key: key.clone(),
        causation: WorkflowPermissionCausation {
            run_id: key.run_id,
            node_id: key.node_id,
            attempt_id: key.attempt_id,
        },
        deadline_unix_ms: 1,
        prepare_abort: None,
        abort_waiter: None,
        abort_requested: true,
        abort_status: None,
        delivery_finished: true,
        late_execution: Some(execution),
        late_shutdown_waiter: Some(waiter),
        stop: None,
    };
    drop(pending);

    release.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), finished_rx)
        .await
        .expect("late shutdown supervisor was aborted with the pending record")
        .unwrap();
}
