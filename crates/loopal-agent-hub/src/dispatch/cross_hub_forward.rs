//! Caller-side cross-hub spawn forwarding: pre-flight checks
//! (schema, name encoding, uplink) → atomic budget-check + shadow
//! pre-registration → `meta/spawn` IPC → rollback on failure.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::hub::Hub;
use crate::uplink::HubUplink;

/// Validated cross-hub spawn pre-flight (schema + name encoding + uplink).
/// Budget check and shadow registration happen atomically AFTER this.
struct ForwardPreflight {
    name: String,
    uplink: Arc<HubUplink>,
    hub_name: String,
}

fn check_payload_and_names(params: &Value, from_agent: &str) -> Result<String, String> {
    // Defense-in-depth: cross-hub spawn must not carry filesystem-coupled
    // fields. Reject (don't silently strip) so client-side bugs surface
    // immediately rather than producing surprising behavior on the receiver.
    loopal_ipc::cross_hub::validate_spawn_payload(params)?;

    let name = params["name"]
        .as_str()
        .ok_or("missing 'name' field")?
        .to_string();

    // Reject '/' in agent names: QualifiedAddress encodes hub/agent as
    // slash-joined string, so a name like "foo/bar" produces ambiguous
    // round-trip — receiver parses caller's `hub-a/foo/bar` as
    // hub=["hub-a","foo"], agent="bar". Forbid it at the cross-hub edge.
    if name.contains('/') {
        return Err(format!(
            "agent name '{name}' cannot contain '/' (cross-hub address encoding)"
        ));
    }
    if from_agent.contains('/') {
        return Err(format!(
            "caller agent name '{from_agent}' cannot contain '/' (cross-hub address encoding)"
        ));
    }
    Ok(name)
}

async fn preflight(
    hub: &Arc<Mutex<Hub>>,
    params: &Value,
    from_agent: &str,
) -> Result<ForwardPreflight, String> {
    let name = check_payload_and_names(params, from_agent)?;
    let h = hub.lock().await;
    let uplink = h
        .uplink
        .clone()
        .ok_or("target_hub specified but no MetaHub uplink")?;
    let hub_name = uplink.hub_name().to_string();
    Ok(ForwardPreflight {
        name,
        uplink,
        hub_name,
    })
}

pub(super) async fn forward_cross_hub_spawn(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    from_agent: &str,
) -> Result<Value, String> {
    let pf = preflight(hub, &params, from_agent).await?;

    let mut spawn_params = params.clone();
    if let Some(obj) = spawn_params.as_object_mut() {
        // Encode parent so the receiving hub can route completions back to
        // this hub's local caller via MetaHub.
        let parent_addr = loopal_protocol::QualifiedAddress::remote([pf.hub_name], from_agent);
        obj.insert("parent".into(), json!(parent_addr.to_string()));
    }

    // Atomic budget check + shadow registration: holding the same lock
    // across both prevents two concurrent cross-hub spawns from each
    // observing budget=N-1 and overshooting. Pre-registering before the
    // IPC also closes the race where a fast-completing remote child's
    // envelope arrives before the spawn response — `emit_agent_finished`
    // would otherwise return None (no entry for the child), and the
    // parent's local completion_tx would never receive the agent-result
    // envelope. With the shadow present, emit_agent_finished can read the
    // shadow's `info.parent` and route the envelope to the local parent.
    {
        let mut h = hub.lock().await;
        let sub_count = h.registry.sub_agent_count();
        if sub_count >= h.max_total_agents as usize {
            return Err(format!(
                "Spawn budget exhausted ({sub_count}/{} sub-agents). \
                 Complete the task with your own tools.",
                h.max_total_agents
            ));
        }
        h.registry.register_shadow(
            &pf.name,
            loopal_protocol::QualifiedAddress::local(from_agent),
        )?;
    }

    let result = pf.uplink.spawn_agent(spawn_params).await;

    match &result {
        Ok(resp) => {
            // Emit SubAgentSpawned routed to the parent so the parent's
            // ViewStateReducer adds the new shadow child to its `children`.
            let agent_id = resp["agent_id"].as_str().unwrap_or("unknown").to_string();
            let model = params["model"].as_str().map(String::from);
            let parent_addr = loopal_protocol::QualifiedAddress::local(from_agent);
            let event = AgentEvent::named(
                parent_addr.clone(),
                AgentEventPayload::SubAgentSpawned {
                    name: pf.name.clone(),
                    agent_id,
                    parent: Some(parent_addr),
                    model,
                    session_id: None,
                },
            );
            let h = hub.lock().await;
            if h.registry.event_sender().try_send(event).is_err() {
                tracing::warn!(
                    agent = %pf.name,
                    "SubAgentSpawned event dropped (channel full, cross-hub)"
                );
            }
        }
        Err(_) => {
            // Roll back the shadow if spawn failed; no remote child exists,
            // so no completion will ever arrive to clean it up.
            let mut h = hub.lock().await;
            h.registry.unregister_connection(&pf.name);
        }
    }
    result
}
