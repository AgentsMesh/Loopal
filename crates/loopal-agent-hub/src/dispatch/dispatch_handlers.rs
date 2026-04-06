//! Hub request handlers — `hub/*` method implementations.

use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_protocol::{Envelope, QualifiedAddress};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tracing::info;

use crate::hub::Hub;
use crate::routing;

pub async fn handle_route(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let envelope: Envelope =
        serde_json::from_value(params).map_err(|e| format!("invalid envelope: {e}"))?;

    let addr = QualifiedAddress::parse(&envelope.target);

    // Remote address or local miss → try uplink
    if addr.is_remote() {
        return route_via_uplink(hub, &envelope).await;
    }

    // Local lookup
    let result = {
        let h = hub.lock().await;
        h.registry
            .get_agent_connection(&addr.agent)
            .map(|conn| (conn, h.registry.event_sender()))
    };

    match result {
        Some((conn, event_tx)) => {
            routing::route_to_agent(&conn, &envelope, &event_tx).await?;
            Ok(json!({"ok": true}))
        }
        None => {
            // Local miss — escalate to MetaHub if uplink exists
            route_via_uplink(hub, &envelope).await
        }
    }
}

/// Forward an envelope to MetaHub via uplink. Errors if no uplink.
async fn route_via_uplink(hub: &Arc<Mutex<Hub>>, envelope: &Envelope) -> Result<Value, String> {
    let uplink = {
        let h = hub.lock().await;
        h.uplink.clone()
    };
    match uplink {
        Some(ul) => {
            ul.route(envelope).await?;
            Ok(json!({"ok": true}))
        }
        None => Err(format!(
            "agent '{}' not found locally and no MetaHub uplink configured",
            envelope.target
        )),
    }
}

pub async fn handle_list_agents(hub: &Arc<Mutex<Hub>>) -> Result<Value, String> {
    let agents: Vec<String> = hub.lock().await.registry.agents.keys().cloned().collect();
    Ok(json!({"agents": agents}))
}

pub async fn handle_control(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let target = params["target"].as_str().ok_or("missing 'target' field")?;
    let command = params["command"].clone();
    let conn = {
        let h = hub.lock().await;
        h.registry
            .get_agent_connection(target)
            .ok_or_else(|| format!("no agent: '{target}'"))?
    };
    conn.send_request(methods::AGENT_CONTROL.name, command)
        .await
        .map_err(|e| format!("control to '{target}' failed: {e}"))?;
    Ok(json!({"ok": true}))
}

pub async fn handle_interrupt(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let target = params["target"].as_str().ok_or("missing 'target' field")?;
    tracing::info!(target, "handle_interrupt: looking up agent connection");
    let conn = {
        let h = hub.lock().await;
        h.registry
            .get_agent_connection(target)
            .ok_or_else(|| format!("no agent: '{target}'"))?
    };
    let result = conn
        .send_notification(methods::AGENT_INTERRUPT.name, json!({}))
        .await;
    match &result {
        Ok(()) => tracing::info!(target, "handle_interrupt: notification sent"),
        Err(e) => tracing::warn!(target, error = %e, "handle_interrupt: send failed"),
    }
    let _ = result;
    Ok(json!({"ok": true}))
}

pub async fn handle_shutdown_agent(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let target = params["target"].as_str().ok_or("missing 'target' field")?;
    let conn = {
        let h = hub.lock().await;
        h.registry
            .get_agent_connection(target)
            .ok_or_else(|| format!("no agent: '{target}'"))?
    };
    // Send shutdown request to the agent — it will close its loop and disconnect.
    let _ = conn
        .send_request(methods::AGENT_SHUTDOWN.name, json!({}))
        .await;
    Ok(json!({"ok": true}))
}

// ── Spawn + wait ──────────────────────────────────────────────────────
pub async fn handle_spawn_agent(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    from_agent: &str,
) -> Result<Value, String> {
    // Cross-hub spawn: delegate to MetaHub if target_hub specified
    if params.get("target_hub").and_then(|v| v.as_str()).is_some() {
        let (uplink, hub_name) = {
            let h = hub.lock().await;
            (
                h.uplink.clone(),
                h.uplink.as_ref().map(|u| u.hub_name().to_string()),
            )
        };
        let result = match (uplink, hub_name) {
            (Some(ul), Some(hn)) => {
                let mut spawn_params = params.clone();
                if let Some(obj) = spawn_params.as_object_mut() {
                    obj.insert("parent".into(), json!(format!("{hn}/{from_agent}")));
                }
                ul.spawn_agent(spawn_params).await
            }
            _ => Err("target_hub specified but no MetaHub uplink".to_string()),
        };
        // On success, register a shadow entry so wait_agent can work locally.
        // The completion will arrive via MetaHub → uplink → agent/message.
        if let Ok(ref resp) = result
            && let Some(name) = resp["name"].as_str()
        {
            let mut h = hub.lock().await;
            h.registry.register_shadow(name, from_agent);
        }
        return result;
    }

    // Local spawn
    let name = params["name"]
        .as_str()
        .ok_or("missing 'name' field")?
        .to_string();
    let cwd = params["cwd"].as_str().unwrap_or(".").to_string();
    let model = params["model"].as_str().map(String::from);
    let prompt = params["prompt"].as_str().map(String::from);
    let permission_mode = params["permission_mode"].as_str().map(String::from);
    let agent_type = params["agent_type"].as_str().map(String::from);
    let depth = params["depth"].as_u64().map(|v| v as u32);

    // Parent: use explicit "parent" field from params if present (cross-hub),
    // otherwise use from_agent (local spawn).
    let parent = params["parent"]
        .as_str()
        .map(String::from)
        .or_else(|| Some(from_agent.to_string()));

    info!(agent = %name, parent = ?parent, "handle_spawn_agent start");
    let hub_clone = hub.clone();
    let name_clone = name.clone();
    let handle = tokio::spawn(async move {
        crate::spawn_manager::spawn_and_register(
            hub_clone,
            name_clone,
            cwd,
            model,
            prompt,
            parent,
            permission_mode,
            agent_type,
            depth,
        )
        .await
    });

    let agent_id = handle
        .await
        .map_err(|e| format!("spawn task failed: {e}"))?
        .map_err(|e| format!("spawn failed: {e}"))?;

    info!(agent = %name, %agent_id, "handle_spawn_agent done");
    Ok(json!({"agent_id": agent_id, "name": name}))
}

pub async fn handle_status(hub: &Arc<Mutex<Hub>>) -> Result<Value, String> {
    let h = hub.lock().await;
    let uplink_info = h.uplink.as_ref().map(|u| {
        json!({
            "connected": true,
            "hub_name": u.hub_name(),
        })
    });
    Ok(json!({
        "agent_count": h.registry.agent_count(),
        "uplink": uplink_info,
    }))
}
