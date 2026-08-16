use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::AgentCompletion;

use super::super::ProductionWorkflowSpawner;
use crate::spawn_manager::spawn::PreparedControl;
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::WorkflowCleanupStatus;

pub(in crate::spawn_manager::workflow) async fn finish_exact(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
    control: &Arc<PreparedControl>,
    completion: AgentCompletion,
    timeout: Duration,
) -> WorkflowCleanupStatus {
    let finish = crate::finish::finish_and_deliver_exact(
        &spawner.hub,
        &execution.address.agent,
        completion,
        &control.connection,
        execution,
    );
    if tokio::time::timeout(timeout, finish).await.is_ok() {
        return WorkflowCleanupStatus::Confirmed;
    }
    tracing::warn!(
        agent = %execution.address,
        generation = execution.connection_generation,
        "workflow exact finish timed out; forcing generation-bound detach"
    );
    let fallback = async {
        let mcp = {
            let mut hub = spawner.hub.lock().await;
            hub.clear_permission_grants(execution);
            hub.spawn_registry.unregister_exact(execution);
            hub.registry.unregister_exact(execution);
            hub.mcp_service.clone()
        };
        mcp.on_agent_detach(execution).await;
        control.connection.close().await;
    };
    if tokio::time::timeout(timeout, fallback).await.is_ok() {
        WorkflowCleanupStatus::Confirmed
    } else {
        tracing::warn!(
            agent = %execution.address,
            generation = execution.connection_generation,
            "workflow generation-bound detach timed out"
        );
        WorkflowCleanupStatus::TimedOut
    }
}
