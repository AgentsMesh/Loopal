use super::ProductionWorkflowSpawner;
use crate::spawn_manager::{PreparedSpawn, SpawnRequestLease};
use crate::workflow::scheduler::WorkflowSpawnRequest;

pub(super) async fn build(
    spawner: &ProductionWorkflowSpawner,
    request: &WorkflowSpawnRequest,
) -> Result<PreparedSpawn, String> {
    let locked = spawner.hub.lock().await;
    let root = locked
        .registry
        .current_execution(&request.owner.root_agent.agent)
        .ok_or_else(|| "workflow root agent is not active".to_string())?;
    if root.address != request.owner.root_agent {
        return Err("workflow owner root address is stale".into());
    }
    let facts = locked
        .registry
        .runtime_facts(&root)
        .ok_or_else(|| "workflow root runtime authority is unavailable".to_string())?;
    if facts.session_id.as_deref() != Some(&request.owner.session_id) {
        return Err("workflow owner session lease is stale".into());
    }
    if facts.depth >= locked.max_agent_depth {
        return Err("workflow child exceeds the configured agent depth".into());
    }
    Ok(PreparedSpawn {
        name: format!("workflow-{}", request.causation.attempt_id),
        request_lease: SpawnRequestLease::Agent(root.clone()),
        cwd: facts.cwd.clone(),
        prompt: Some(super::worker_prompt::build(request)),
        parent: Some(root.address.clone()),
        parent_execution: Some(root),
        authority: request
            .worker_profile
            .intersect_authority(facts.spawn.clone()),
        agent_type: Some(request.worker_profile.agent_type().into()),
        depth: facts.depth + 1,
        fork_context: None,
        workflow_permission_causation: Some(request.causation.clone()),
        workflow_attempt_capability: Some(request.attempt_capability.clone()),
        workflow_completion_result_limit: Some(request.completion_result_limit),
        notify_parent_on_completion: false,
        root_cwd: facts.root_cwd.clone(),
        root: facts.root.clone(),
    })
}
