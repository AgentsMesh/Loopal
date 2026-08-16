use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use tracing::info;

use crate::hub::Hub;
use crate::request_principal::{AgentPrincipal, TrustedMetaHubPrincipal};
use crate::spawn_manager::PreparedSpawn;

#[cfg(test)]
#[path = "spawn_routing_tests.rs"]
mod tests;

pub async fn handle_spawn_agent(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    principal: &AgentPrincipal,
) -> Result<Value, String> {
    if let Some(target) =
        super::spawn_parent_policy::normalize_target_hub_value(params.get("target_hub"))?
    {
        if target.contains('/') {
            return Err(format!(
                "'target_hub' cannot contain '/' (cross-hub address encoding), got: {target}"
            ));
        }
        let (own_hub, max_depth) = {
            let locked = hub.lock().await;
            (
                locked
                    .uplink
                    .as_ref()
                    .map(|uplink| uplink.hub_name().to_string()),
                locked.max_agent_depth,
            )
        };
        if !super::spawn_parent_policy::is_self_target(own_hub.as_deref(), &target) {
            let mut forwarded =
                super::spawn_authority::prepare_cross_hub_payload(&params, principal, max_depth)?;
            forwarded["target_hub"] = Value::String(target);
            return super::cross_hub_forward::forward_cross_hub_spawn(
                hub,
                forwarded,
                &principal.execution,
            )
            .await;
        }
    }
    let max_depth = hub.lock().await.max_agent_depth;
    let mut local = params;
    if let Some(object) = local.as_object_mut() {
        object.remove("target_hub");
    }
    let prepared = super::spawn_authority::prepare_local(&local, principal, max_depth)?;
    spawn_via_manager(hub.clone(), prepared).await
}

pub async fn handle_spawn_remote_agent(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    principal: &TrustedMetaHubPrincipal,
) -> Result<Value, String> {
    let prepared = {
        let locked = hub.lock().await;
        super::spawn_prepare::prepare_remote_spawn(&params, &locked, principal.connection())?
    };
    spawn_via_manager(hub.clone(), prepared).await
}

async fn spawn_via_manager(hub: Arc<Mutex<Hub>>, prepared: PreparedSpawn) -> Result<Value, String> {
    let name = prepared.name.clone();
    info!(agent = %name, parent = ?prepared.parent, "spawn start");
    let agent_id = tokio::spawn(crate::spawn_manager::spawn_and_register(hub, prepared))
        .await
        .map_err(|error| format!("spawn task failed: {error}"))?
        .map_err(|error| format!("spawn failed: {error}"))?;
    info!(agent = %name, %agent_id, "spawn done");
    Ok(json!({"agent_id": agent_id, "name": name}))
}
