use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{WorkflowAttemptId, WorkflowPermissionCausation, WorkflowRunId};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;

use super::journal_support::TestJournal;
use super::support::{TestClock, TestIds};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{
    WorkflowActivationFailure, WorkflowCleanupStatus, WorkflowPreparedWorker,
    WorkflowRecoveryAdoptionError, WorkflowRecoveryAdoptionRequest, WorkflowSpawnFailure,
    WorkflowSpawnRequest, WorkflowSpawner, WorkflowStopStatus, WorkflowWorkerOutcome,
};
use crate::workflow::{WorkflowCoordinator, WorkflowCoordinatorHandle, WorkflowCoordinatorMode};

pub(super) enum SpawnerEffect {
    Prepare {
        request: WorkflowSpawnRequest,
        response: oneshot::Sender<Result<WorkflowPreparedWorker, WorkflowSpawnFailure>>,
    },
    Activate {
        execution: AgentExecutionRef,
        response: oneshot::Sender<Result<(), WorkflowActivationFailure>>,
    },
    Interrupt {
        execution: AgentExecutionRef,
        response: oneshot::Sender<WorkflowStopStatus>,
    },
    Shutdown {
        execution: AgentExecutionRef,
        response: oneshot::Sender<WorkflowCleanupStatus>,
    },
    AbortPrepare {
        causation: WorkflowPermissionCausation,
        response: oneshot::Sender<WorkflowCleanupStatus>,
    },
    AdoptRecovered {
        request: WorkflowRecoveryAdoptionRequest,
        response: oneshot::Sender<Result<WorkflowPreparedWorker, WorkflowRecoveryAdoptionError>>,
    },
}

pub(super) struct TestSpawner {
    pub(super) effects: mpsc::UnboundedSender<SpawnerEffect>,
}

pub(super) struct SpawnerControl {
    effects: Mutex<mpsc::UnboundedReceiver<SpawnerEffect>>,
}

pub(super) fn test_spawner() -> (Arc<TestSpawner>, SpawnerControl) {
    let (effects, receiver) = mpsc::unbounded_channel();
    (
        Arc::new(TestSpawner { effects }),
        SpawnerControl {
            effects: Mutex::new(receiver),
        },
    )
}

pub(super) fn coordinator(
    times: impl IntoIterator<Item = u64>,
    run_ids: impl IntoIterator<Item = WorkflowRunId>,
    attempt_ids: impl IntoIterator<Item = WorkflowAttemptId>,
    journal: Arc<TestJournal>,
    spawner: Arc<dyn WorkflowSpawner>,
) -> (
    WorkflowCoordinatorHandle,
    JoinHandle<()>,
    Arc<TestClock>,
    Arc<TestIds>,
) {
    let clock = Arc::new(TestClock::new(times));
    let ids = Arc::new(TestIds::with_attempts(run_ids, attempt_ids));
    let (handle, task) = WorkflowCoordinator::spawn_for_test_with_spawner(
        WorkflowCoordinatorMode::ExecutionHarness,
        clock.clone(),
        ids.clone(),
        journal,
        spawner,
    );
    (handle, task, clock, ids)
}

impl SpawnerControl {
    pub(super) async fn next(&self) -> SpawnerEffect {
        tokio::time::timeout(Duration::from_secs(5), self.effects.lock().await.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for workflow spawner effect"))
            .unwrap_or_else(|| panic!("workflow spawner closed before the next effect"))
    }

    pub(super) async fn assert_idle(&self) {
        assert!(matches!(
            self.effects.lock().await.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    pub(super) async fn assert_drained(&self) {
        assert!(matches!(
            self.effects.lock().await.try_recv(),
            Err(mpsc::error::TryRecvError::Empty | mpsc::error::TryRecvError::Disconnected)
        ));
    }
}

pub(super) fn prepared_worker(
    agent: &str,
    generation: u64,
) -> (
    WorkflowPreparedWorker,
    oneshot::Sender<WorkflowWorkerOutcome>,
) {
    let (outcome, receiver) = oneshot::channel();
    (
        WorkflowPreparedWorker {
            execution: AgentExecutionRef::local(agent, generation),
            outcome: receiver,
        },
        outcome,
    )
}
