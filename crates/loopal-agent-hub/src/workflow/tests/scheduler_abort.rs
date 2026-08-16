use std::sync::Arc;

use loopal_protocol::{
    WorkflowAttemptId, WorkflowCancelRequest, WorkflowRequestId, WorkflowRunId, WorkflowRunState,
};
use tokio::sync::oneshot;

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, test_spawner};
use super::support::{get_request, owner, request};
use crate::workflow::command::WorkflowCommand;
use crate::workflow::scheduler::{
    AttemptKey, WorkflowActivationFailure, WorkflowCleanupStatus, WorkflowPreparedDelivery,
    WorkflowSpawnFailure,
};

#[test]
fn activation_failure_variants_are_part_of_the_scheduler_contract() {
    let failure = WorkflowSpawnFailure {
        completion: loopal_protocol::AgentCompletion::new("activation_stopped", None),
        failure: loopal_protocol::WorkflowAttemptFailure {
            class: loopal_protocol::WorkflowFailureClass::TransientBeforeExecution,
            reason: "stopped before activation".into(),
        },
    };
    let _ = WorkflowActivationFailure::Stopped(failure.clone());
    let _ = WorkflowActivationFailure::Uncertain(failure.failure);
}

#[tokio::test]
async fn ambiguous_preparation_error_wins_in_either_callback_order() {
    run_ambiguous_abort_order(true).await;
    run_ambiguous_abort_order(false).await;
}

async fn run_ambiguous_abort_order(prepared_first: bool) {
    let suffix = if prepared_first {
        "prepared-first"
    } else {
        "abort-first"
    };
    let run_id = WorkflowRunId::new(format!("wrun_abort_order_{suffix}"));
    let attempt_id = WorkflowAttemptId::new(format!("watt_abort_order_{suffix}"));
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        100..130,
        [run_id.clone()],
        [attempt_id.clone()],
        journal.clone(),
        spawner.clone(),
    );
    let owner = owner(&format!("session-abort-order-{suffix}"), "root");
    let mut start = request(&format!("wreq_abort_order_{suffix}"));
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();
    handle.start(owner.clone(), start).await.unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();
    let SpawnerEffect::Prepare {
        request: spawn,
        response: prepare,
    } = control.next().await
    else {
        panic!("expected preparation")
    };

    handle
        .cancel(
            owner.clone(),
            WorkflowCancelRequest {
                request_id: WorkflowRequestId::new(format!("wreq_abort_order_cancel_{suffix}")),
                run_id: run_id.clone(),
                reason: Some("race callbacks".into()),
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

    let key = AttemptKey {
        run_id: run_id.clone(),
        node_id: spawn.causation.node_id.clone(),
        attempt_id,
    };
    let (started, started_seen) = oneshot::channel();
    let (release, release_waiter) = oneshot::channel();
    handle
        .commands
        .send(WorkflowCommand::Pause {
            started,
            release: release_waiter,
        })
        .await
        .unwrap();
    started_seen.await.unwrap();

    let delivery = WorkflowPreparedDelivery::new(
        Err(WorkflowSpawnFailure {
            completion: loopal_protocol::AgentCompletion::new("prepare_ambiguous", None),
            failure: loopal_protocol::WorkflowAttemptFailure {
                class: loopal_protocol::WorkflowFailureClass::AmbiguousExecution,
                reason: "preparation result arrived during abort".into(),
            },
        }),
        spawner,
    );
    if prepared_first {
        handle
            .commands
            .send(WorkflowCommand::WorkerPrepared {
                owner: owner.clone(),
                key: key.clone(),
                prepared: delivery,
            })
            .await
            .unwrap();
        handle
            .commands
            .send(WorkflowCommand::WorkerPreparationAborted {
                owner: owner.clone(),
                key: key.clone(),
                status: WorkflowCleanupStatus::Confirmed,
            })
            .await
            .unwrap();
    } else {
        handle
            .commands
            .send(WorkflowCommand::WorkerPreparationAborted {
                owner: owner.clone(),
                key: key.clone(),
                status: WorkflowCleanupStatus::Confirmed,
            })
            .await
            .unwrap();
        handle
            .commands
            .send(WorkflowCommand::WorkerPrepared {
                owner: owner.clone(),
                key,
                prepared: delivery,
            })
            .await
            .unwrap();
    }
    let _ = release.send(());
    journal.wait_for_event_batches(4).await;
    let run = handle
        .get(
            owner.clone(),
            get_request(&format!("wreq_abort_order_get_{suffix}"), run_id),
        )
        .await
        .unwrap()
        .run
        .unwrap();
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(
        run.failure.unwrap().class,
        loopal_protocol::WorkflowFailureClass::AmbiguousExecution
    );

    let _ = abort.send(WorkflowCleanupStatus::Confirmed);
    drop(prepare);
    drop(handle);
    task.await.unwrap();
    control.assert_drained().await;
}
