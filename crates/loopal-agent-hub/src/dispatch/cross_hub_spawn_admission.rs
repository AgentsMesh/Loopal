use std::sync::Arc;

use serde_json::Value;
use tokio::sync::Mutex;

use crate::authoritative_events::AuthoritativeEventSink;
use crate::spawn_manager::authority_audit::SpawnAudit;
use crate::types::AgentExecutionRef;
use crate::{Hub, HubUplink};

pub(super) struct CrossHubSpawnAdmission {
    pub(super) event_sink: AuthoritativeEventSink,
    pub(super) parent_generation: Option<u64>,
    pub(super) shadow_generation: u64,
}

pub(super) async fn audit_and_register_shadow(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    params: &Value,
    requester: &AgentExecutionRef,
    uplink: &Arc<HubUplink>,
    notify_parent_on_completion: bool,
) -> Result<CrossHubSpawnAdmission, String> {
    let audit = {
        let mut locked = hub.lock().await;
        validate(&mut locked, name, requester, uplink)?;
        let target_hub = params["target_hub"]
            .as_str()
            .ok_or_else(|| "cross-hub spawn target must be a string".to_string())?;
        SpawnAudit::for_cross_hub(&locked, name, target_hub, requester, params)?
    };
    audit.append().await?;

    let mut locked = hub.lock().await;
    validate(&mut locked, name, requester, uplink)?;
    let shadow = locked
        .registry
        .register_shadow_with_parent_policy_execution(
            name,
            loopal_protocol::QualifiedAddress::local(&requester.address.agent),
            notify_parent_on_completion,
        )?;
    locked.install_shadow_spawn_admission(name, shadow.connection_generation, uplink.clone());
    Ok(CrossHubSpawnAdmission {
        event_sink: AuthoritativeEventSink::from_hub(&locked),
        parent_generation: Some(requester.connection_generation),
        shadow_generation: shadow.connection_generation,
    })
}

fn validate(
    hub: &mut Hub,
    name: &str,
    requester: &AgentExecutionRef,
    uplink: &Arc<HubUplink>,
) -> Result<(), String> {
    if !hub.registry.owns_active_lease(requester) {
        return Err("spawn requester connection lease is stale".into());
    }
    if !hub
        .uplink
        .as_ref()
        .is_some_and(|active| Arc::ptr_eq(active, uplink))
    {
        return Err("MetaHub uplink changed during remote spawn admission".into());
    }
    if hub.shadow_name_is_quarantined(name, uplink) {
        return Err(format!(
            "remote agent name '{name}' is quarantined until the MetaHub uplink reconnects"
        ));
    }
    hub.registry.validate_shadow_registration(name)?;
    let sub_count = hub.registry.sub_agent_count();
    if sub_count >= hub.max_total_agents as usize {
        return Err(format!(
            "Spawn budget exhausted ({sub_count}/{} sub-agents). \
             Complete the task with your own tools.",
            hub.max_total_agents
        ));
    }
    Ok(())
}
