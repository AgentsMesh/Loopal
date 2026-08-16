use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::AgentCompletion;

use super::super::ProductionWorkflowSpawner;

pub(super) async fn terminate_unowned(
    spawner: &ProductionWorkflowSpawner,
    owner: &crate::workflow::WorkflowOwner,
    causation: &loopal_protocol::WorkflowPermissionCausation,
    control: Arc<crate::spawn_manager::spawn::PreparedControl>,
    execution: crate::types::AgentExecutionRef,
    process: crate::spawn_manager::spawn::WorkflowProcessOwner,
) {
    const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
    let audit = super::super::lifecycle_audit::append_before_cleanup(
        spawner,
        owner,
        causation,
        Some(&execution),
        super::super::lifecycle_audit::WorkflowAuditPhase::Shutdown,
    );
    let _ = tokio::time::timeout(CLEANUP_TIMEOUT, audit).await;
    let _ = tokio::time::timeout(CLEANUP_TIMEOUT, process.shutdown()).await;
    let _ = super::super::control::finish_exact(
        spawner,
        &execution,
        &control,
        AgentCompletion::new("workflow_prepare_cancelled", None),
        CLEANUP_TIMEOUT,
    )
    .await;
}
