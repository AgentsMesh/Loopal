use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::WorkflowPermissionCausation;
use tokio::task::JoinHandle;

use super::WorkflowSpawner;
use crate::types::AgentExecutionRef;

const WORKFLOW_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkflowCleanupStatus {
    Confirmed,
    TimedOut,
}

pub(in crate::workflow) async fn bounded_abort_prepare(
    spawner: Arc<dyn WorkflowSpawner>,
    causation: &WorkflowPermissionCausation,
) -> WorkflowCleanupStatus {
    tokio::time::timeout(
        WORKFLOW_CLEANUP_TIMEOUT,
        spawner.abort_prepare_and_wait(causation, WORKFLOW_CLEANUP_TIMEOUT),
    )
    .await
    .unwrap_or(WorkflowCleanupStatus::TimedOut)
}

pub(in crate::workflow) async fn bounded_shutdown(
    spawner: Arc<dyn WorkflowSpawner>,
    execution: &AgentExecutionRef,
) -> WorkflowCleanupStatus {
    tokio::time::timeout(
        WORKFLOW_CLEANUP_TIMEOUT,
        spawner.shutdown_and_wait(execution, WORKFLOW_CLEANUP_TIMEOUT),
    )
    .await
    .unwrap_or(WorkflowCleanupStatus::TimedOut)
}

pub(in crate::workflow) async fn abort_local_preparation(task: JoinHandle<()>) {
    task.abort();
    let _ = tokio::time::timeout(WORKFLOW_CLEANUP_TIMEOUT, task).await;
}
