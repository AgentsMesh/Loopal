use std::sync::Arc;

use loopal_protocol::{
    WorkflowCancelRequest, WorkflowEventPayload, WorkflowRequestId, WorkflowRunId, WorkflowRunState,
};
use tokio::sync::oneshot;

use super::journal_support::TestJournal;
use super::scheduler_reconnect_support::{begin_handshake, coordinator, recovered};
use super::scheduler_support::{SpawnerControl, SpawnerEffect, prepared_worker};
use super::support::{TestClock, owner};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{
    WorkflowCleanupStatus, WorkflowStopStatus, WorkflowWorkerOutcome,
};
use crate::workflow::{WorkflowCoordinatorHandle, WorkflowOwner};

#[tokio::test]
async fn adopted_attempt_deadline_interrupts_then_shuts_down_exact_execution() {
    let fixture = adopted([800, 801, 802]).await;
    fixture.handle.tick(fixture.deadline).await.unwrap();
    let SpawnerEffect::Interrupt {
        execution,
        response,
    } = fixture.control.next().await
    else {
        panic!("expected recovered attempt interrupt")
    };
    assert_eq!(execution, fixture.execution);
    response.send(WorkflowStopStatus::Requested).unwrap();

    fixture.handle.tick(fixture.deadline + 1_000).await.unwrap();
    let SpawnerEffect::Shutdown {
        execution,
        response,
    } = fixture.control.next().await
    else {
        panic!("expected recovered attempt shutdown")
    };
    assert_eq!(execution, fixture.execution);
    response.send(WorkflowCleanupStatus::Confirmed).unwrap();
    fixture.journal.wait_for_event_batches(2).await;
    assert!(matches!(
        last_payload(&fixture.journal),
        WorkflowEventPayload::AttemptFailed { .. }
    ));
    assert_eq!(
        state(&fixture.handle, fixture.owner.clone()).await,
        WorkflowRunState::Failed
    );
    assert!(fixture.outcome.is_closed());
    finish(fixture).await;
}

#[tokio::test]
async fn adopted_attempt_cancel_owns_interrupt_and_shutdown_lifecycle() {
    let fixture = adopted([800, 801, 802, 803, 804]).await;
    fixture
        .handle
        .cancel(
            fixture.owner.clone(),
            WorkflowCancelRequest {
                request_id: WorkflowRequestId::new("wreq_recovered_cancel"),
                run_id: fixture.run_id.clone(),
                reason: Some("cancel recovered worker".into()),
            },
        )
        .await
        .unwrap();
    let SpawnerEffect::Interrupt {
        execution,
        response,
    } = fixture.control.next().await
    else {
        panic!("expected recovered attempt interrupt")
    };
    assert_eq!(execution, fixture.execution);
    response.send(WorkflowStopStatus::Requested).unwrap();
    fixture.handle.tick(1_803).await.unwrap();
    let SpawnerEffect::Shutdown {
        execution,
        response,
    } = fixture.control.next().await
    else {
        panic!("expected recovered attempt shutdown")
    };
    assert_eq!(execution, fixture.execution);
    response.send(WorkflowCleanupStatus::Confirmed).unwrap();
    fixture.journal.wait_for_event_batches(2).await;
    assert!(matches!(
        last_payload(&fixture.journal),
        WorkflowEventPayload::AttemptCancelled { .. }
    ));
    assert_eq!(
        state(&fixture.handle, fixture.owner.clone()).await,
        WorkflowRunState::Cancelled
    );
    assert!(fixture.outcome.is_closed());
    finish(fixture).await;
}

struct Fixture {
    handle: WorkflowCoordinatorHandle,
    task: tokio::task::JoinHandle<()>,
    control: SpawnerControl,
    journal: Arc<TestJournal>,
    owner: WorkflowOwner,
    run_id: WorkflowRunId,
    execution: AgentExecutionRef,
    outcome: oneshot::Sender<WorkflowWorkerOutcome>,
    deadline: u64,
}

async fn adopted(times: impl IntoIterator<Item = u64>) -> Fixture {
    let (run, causation, capability) = recovered(true);
    let deadline = run.attempts[0]
        .dispatched_at_unix_ms
        .saturating_add(run.spec.limits.attempt_timeout_ms);
    let run_id = run.id.clone();
    let journal = Arc::new(TestJournal::new());
    let (handle, task, control) =
        coordinator(journal.clone(), Arc::new(TestClock::new(times)), run);
    let owner = owner("session-adopt-control", "root");
    handle.recover(owner.clone()).await.unwrap();
    let execution = AgentExecutionRef::local("worker", 7);
    let handshake = tokio::spawn(begin_handshake(
        handle.clone(),
        owner.clone(),
        causation,
        capability,
        execution.clone(),
    ));
    let SpawnerEffect::AdoptRecovered { response, .. } = control.next().await else {
        panic!("expected recovery custody acquisition")
    };
    let (worker, outcome) = prepared_worker("worker", 7);
    assert!(response.send(Ok(worker)).is_ok());
    handshake.await.unwrap().unwrap();
    Fixture {
        handle,
        task,
        control,
        journal,
        owner,
        run_id,
        execution,
        outcome,
        deadline,
    }
}

async fn state(handle: &WorkflowCoordinatorHandle, owner: WorkflowOwner) -> WorkflowRunState {
    handle.snapshot(owner).await.unwrap().recent[0].state
}

fn last_payload(journal: &TestJournal) -> WorkflowEventPayload {
    journal
        .events()
        .into_iter()
        .last()
        .unwrap()
        .2
        .last()
        .unwrap()
        .payload
        .clone()
}

async fn finish(fixture: Fixture) {
    fixture.control.assert_idle().await;
    fixture.handle.shutdown().await.unwrap();
    fixture.task.await.unwrap();
}
