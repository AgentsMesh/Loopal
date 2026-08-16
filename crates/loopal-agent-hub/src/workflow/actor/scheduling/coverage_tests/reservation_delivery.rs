use std::future;

use loopal_protocol::{WorkflowAttemptCapability, WorkflowAttemptId, WorkflowRunId};

use super::*;
use crate::workflow::WorkflowIdSource;

struct FixedAttemptId(WorkflowAttemptId);

impl WorkflowIdSource for FixedAttemptId {
    fn next_run_id(&self) -> WorkflowRunId {
        WorkflowRunId::new("wrun_unused")
    }

    fn next_attempt_id(&self) -> WorkflowAttemptId {
        self.0.clone()
    }

    fn next_attempt_capability(&self) -> WorkflowAttemptCapability {
        WorkflowAttemptCapability::parse("66".repeat(32)).unwrap()
    }
}

struct PendingSpawner;

#[async_trait::async_trait]
impl WorkflowSpawner for PendingSpawner {
    async fn prepare(
        &self,
        _request: WorkflowSpawnRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure> {
        future::pending().await
    }

    async fn abort_prepare_and_wait(
        &self,
        _causation: &WorkflowPermissionCausation,
        _timeout: Duration,
    ) -> WorkflowCleanupStatus {
        WorkflowCleanupStatus::Confirmed
    }

    async fn adopt_recovered(
        &self,
        _request: WorkflowRecoveryAdoptionRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowRecoveryAdoptionError> {
        Err(WorkflowRecoveryAdoptionError::MissingCustody)
    }

    async fn activate(
        &self,
        _execution: &AgentExecutionRef,
    ) -> Result<(), WorkflowActivationFailure> {
        Ok(())
    }

    async fn interrupt(&self, _execution: &AgentExecutionRef) -> WorkflowStopStatus {
        WorkflowStopStatus::Stopped
    }

    async fn shutdown_and_wait(
        &self,
        _execution: &AgentExecutionRef,
        _timeout: Duration,
    ) -> WorkflowCleanupStatus {
        WorkflowCleanupStatus::Confirmed
    }
}

fn delivery_coordinator(
    run: WorkflowRunSnapshot,
    id: &str,
    spawner: Arc<dyn WorkflowSpawner>,
) -> (WorkflowCoordinator, mpsc::Sender<WorkflowCommand>) {
    let (mut coordinator, commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner,
        20,
    );
    coordinator.ids = Arc::new(FixedAttemptId(WorkflowAttemptId::new(id)));
    (coordinator, commands)
}

#[tokio::test(start_paused = true)]
async fn preparation_timeout_tolerates_a_dropped_callback_owner() {
    let owner = owner();
    let run = running_ready_run("wrun_reserve_timeout");
    let run_id = run.id.clone();
    let id = WorkflowAttemptId::new("watt_reserve_timeout");
    let (mut coordinator, commands) =
        delivery_coordinator(run, id.as_str(), Arc::new(PendingSpawner));
    drop(commands);

    dispatch::admit(&mut coordinator, owner, run_id)
        .await
        .unwrap();
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(101)).await;
    let prepare = coordinator
        .pending
        .get_mut(&id)
        .unwrap()
        .prepare_abort
        .take()
        .unwrap();
    prepare.await.unwrap();
}

#[tokio::test]
async fn prepared_delivery_tolerates_a_closed_callback_receiver() {
    let owner = owner();
    let run = running_ready_run("wrun_reserve_closed");
    let run_id = run.id.clone();
    let id = WorkflowAttemptId::new("watt_reserve_closed");
    let (mut coordinator, _commands) =
        delivery_coordinator(run, id.as_str(), TestSpawner::confirmed());
    coordinator.commands.close();

    dispatch::admit(&mut coordinator, owner, run_id)
        .await
        .unwrap();
    let prepare = coordinator
        .pending
        .get_mut(&id)
        .unwrap()
        .prepare_abort
        .take()
        .unwrap();
    prepare.await.unwrap();
}
