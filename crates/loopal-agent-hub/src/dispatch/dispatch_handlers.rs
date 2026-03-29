//! Hub request handlers — `hub/*` method implementations.

use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tracing::info;

use crate::hub::Hub;
use crate::routing;

pub async fn handle_route(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let envelope: Envelope =
        serde_json::from_value(params).map_err(|e| format!("invalid envelope: {e}"))?;
    let (conn, event_tx) = {
        let h = hub.lock().await;
        let conn = h
            .registry
            .get_agent_connection(&envelope.target)
            .ok_or_else(|| format!("no agent registered: '{}'", envelope.target))?;
        (conn, h.registry.event_sender())
    };
    routing::route_to_agent(&conn, &envelope, &event_tx).await?;
    Ok(json!({"ok": true}))
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
    let conn = {
        let h = hub.lock().await;
        h.registry
            .get_agent_connection(target)
            .ok_or_else(|| format!("no agent: '{target}'"))?
    };
    let _ = conn
        .send_notification(methods::AGENT_INTERRUPT.name, json!({}))
        .await;
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
    let name = params["name"]
        .as_str()
        .ok_or("missing 'name' field")?
        .to_string();
    let cwd = params["cwd"].as_str().unwrap_or(".").to_string();
    let model = params["model"].as_str().map(String::from);
    let prompt = params["prompt"].as_str().map(String::from);
    let parent = Some(from_agent.to_string());

    info!(agent = %name, parent = %from_agent, "handle_spawn_agent start");
    let hub_clone = hub.clone();
    let name_clone = name.clone();
    let handle = tokio::spawn(async move {
        crate::spawn_manager::spawn_and_register(hub_clone, name_clone, cwd, model, prompt, parent)
            .await
    });

    let agent_id = handle
        .await
        .map_err(|e| format!("spawn task failed: {e}"))?
        .map_err(|e| format!("spawn failed: {e}"))?;

    info!(agent = %name, %agent_id, "handle_spawn_agent done");
    Ok(json!({"agent_id": agent_id, "name": name}))
}
