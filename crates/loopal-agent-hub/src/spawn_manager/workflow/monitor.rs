use std::sync::Arc;

use loopal_protocol::{AgentCompletion, WorkflowOutputContract};
use tokio::sync::oneshot;

use super::ProductionWorkflowSpawner;
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::WorkflowWorkerOutcome;

pub(super) fn spawn(
    spawner: &ProductionWorkflowSpawner,
    execution: AgentExecutionRef,
    control: Arc<super::super::spawn::PreparedControl>,
    outcome: oneshot::Sender<WorkflowWorkerOutcome>,
    contract: Option<WorkflowOutputContract>,
) {
    let hub = spawner.hub.clone();
    let attempts = spawner.attempts.clone();
    let changed = spawner.changed.clone();
    tokio::spawn(async move {
        let completion = wait_completion(hub, &execution, &control.connection).await;
        let _ = outcome.send(super::outcome::worker(completion, contract));
        let mut owners = attempts.lock().await;
        let cleanup_owns = owners
            .by_execution
            .get(&execution)
            .and_then(|attempt| owners.by_attempt.get(attempt))
            .is_some_and(|owner| owner.execution == execution && owner.cleanup_registered);
        if !cleanup_owns {
            super::remove_exact_owner(&mut owners, &execution);
        }
        drop(owners);
        changed.notify_waiters();
    });
}

pub(in crate::spawn_manager::workflow) async fn wait_completion(
    hub: Arc<tokio::sync::Mutex<crate::hub::Hub>>,
    execution: &AgentExecutionRef,
    connection: &Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
) -> AgentCompletion {
    let Some(mut completion) = hub.lock().await.registry.watch_completion_exact(execution) else {
        return AgentCompletion::new(
            "transport_error",
            Some("workflow execution lease is stale".into()),
        );
    };
    loop {
        if let Some(value) = completion.borrow().clone() {
            return value;
        }
        if !connection.is_connected() || completion.changed().await.is_err() {
            return AgentCompletion::new(
                "transport_error",
                Some("workflow worker exited before exact completion".into()),
            );
        }
    }
}
