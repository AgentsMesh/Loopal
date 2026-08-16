use std::sync::Arc;

use tokio::sync::Mutex;

use crate::hub::Hub;

use super::PreparedSpawn;

pub(super) async fn authorized<T>(
    hub: &Arc<Mutex<Hub>>,
    prepared: &PreparedSpawn,
    fork: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let audit = {
        let locked = hub.lock().await;
        validate(&locked, prepared)?;
        super::authority_audit::SpawnAudit::for_prepared(&locked, prepared)?
    };
    audit.append().await?;
    // The final lease check and the synchronous fork are one admission
    // critical section.  No await or Hub I/O occurs while this guard is held;
    // this prevents a same-name reconnect between validation and process
    // creation from stealing the prepared request.
    let locked = hub.lock().await;
    validate(&locked, prepared)?;
    fork()
}

fn validate(hub: &Hub, prepared: &PreparedSpawn) -> Result<(), String> {
    if !prepared.request_lease.is_current(hub) {
        return Err("spawn requester connection lease is stale".into());
    }
    if prepared
        .parent_execution
        .as_ref()
        .is_some_and(|parent| !hub.registry.owns_active_lease(parent))
    {
        return Err("spawn parent connection lease is stale".into());
    }
    let count = hub.registry.sub_agent_count();
    if count >= hub.max_total_agents as usize {
        return Err(format!(
            "Spawn budget exhausted ({count}/{} sub-agents). Complete the task with your own tools.",
            hub.max_total_agents
        ));
    }
    if hub.registry.agent_info(&prepared.name).is_some() {
        return Err(format!("agent '{}' already registered", prepared.name));
    }
    Ok(())
}
