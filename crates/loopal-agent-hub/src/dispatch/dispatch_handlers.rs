//! Hub request handlers — `hub/*` method implementations.

use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::Hub;
use crate::routing;

pub async fn handle_route(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let envelope: Envelope =
        serde_json::from_value(params).map_err(|e| format!("invalid envelope: {e}"))?;

    // Remote address → uplink immediately (target carries the next hop).
    if envelope.target.is_remote() {
        return route_via_uplink(hub, &envelope).await;
    }

    // Local lookup
    let result = {
        let h = hub.lock().await;
        h.registry
            .get_agent_connection(&envelope.target.agent)
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
    let agents: Vec<Value> = hub
        .lock()
        .await
        .registry
        .list_agents()
        .into_iter()
        .map(|(name, state)| json!({"name": name, "state": state}))
        .collect();
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
