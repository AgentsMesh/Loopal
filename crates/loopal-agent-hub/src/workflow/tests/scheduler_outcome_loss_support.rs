use std::sync::Arc;

use loopal_protocol::{WorkflowAttemptId, WorkflowCancelRequest, WorkflowRequestId, WorkflowRunId};

use super::journal_support::TestJournal;
use super::scheduler_support::{
    SpawnerControl, SpawnerEffect, coordinator, prepared_worker, test_spawner,
};
use super::support::{get_request, owner, request};
use crate::types::AgentExecutionRef;
use crate::workflow::command::WorkflowCommand;
use crate::workflow::scheduler::AttemptKey;
use crate::workflow::{WorkflowCoordinatorHandle, WorkflowOwner};

pub(super) struct RunningFixture {
    pub(super) handle: WorkflowCoordinatorHandle,
    pub(super) task: tokio::task::JoinHandle<()>,
    pub(super) journal: Arc<TestJournal>,
    pub(super) control: SpawnerControl,
    owner: WorkflowOwner,
    key: AttemptKey,
    pub(super) execution: AgentExecutionRef,
    pub(super) outcome:
        Option<tokio::sync::oneshot::Sender<crate::workflow::scheduler::WorkflowWorkerOutcome>>,
}

impl RunningFixture {
    pub(super) async fn cancel(&self) {
        self.handle
            .cancel(
                self.owner.clone(),
                WorkflowCancelRequest {
                    request_id: WorkflowRequestId::new(format!("cancel_{}", self.key.attempt_id)),
                    run_id: self.key.run_id.clone(),
                    reason: Some("stop now".into()),
                },
            )
            .await
            .unwrap();
    }

    pub(super) async fn deliver_lost_barrier(&self) {
        self.handle
            .commands
            .send(WorkflowCommand::WorkerOutcomeLost {
                owner: self.owner.clone(),
                key: self.key.clone(),
                execution: self.execution.clone(),
            })
            .await
            .unwrap();
        let _ = self.run("lost_barrier").await;
    }

    pub(super) async fn run(&self, suffix: &str) -> loopal_protocol::WorkflowRunSnapshot {
        self.handle
            .get(
                self.owner.clone(),
                get_request(
                    &format!("get_{suffix}_{}", self.key.attempt_id),
                    self.key.run_id.clone(),
                ),
            )
            .await
            .unwrap()
            .run
            .unwrap()
    }

    pub(super) async fn finish(self) {
        drop(self.handle);
        self.task.await.unwrap();
    }
}

pub(super) async fn running_attempt(label: &str, generation: u64) -> RunningFixture {
    let run_id = WorkflowRunId::new(format!("wrun_outcome_{label}"));
    let attempt_id = WorkflowAttemptId::new(format!("watt_outcome_{label}"));
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        100..130,
        [run_id.clone()],
        [attempt_id.clone()],
        journal.clone(),
        spawner,
    );
    let owner = owner(&format!("session-outcome-{label}"), "root");
    let mut start = request(&format!("start_outcome_{label}"));
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();
    handle.start(owner.clone(), start).await.unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();
    let SpawnerEffect::Prepare { response, .. } = control.next().await else {
        panic!("expected preparation")
    };
    let (worker, outcome) = prepared_worker("worker", generation);
    assert!(response.send(Ok(worker)).is_ok());
    let SpawnerEffect::Activate {
        execution,
        response,
    } = control.next().await
    else {
        panic!("expected activation")
    };
    assert!(response.send(Ok(())).is_ok());
    journal.wait_for_event_batches(4).await;
    RunningFixture {
        handle,
        task,
        journal,
        control,
        owner,
        key: AttemptKey {
            run_id,
            node_id: "output".into(),
            attempt_id,
        },
        execution,
        outcome: Some(outcome),
    }
}
