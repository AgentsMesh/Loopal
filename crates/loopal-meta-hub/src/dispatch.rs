use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use loopal_ipc::protocol::methods;

use crate::meta_hub::MetaHub;

/// Dispatch a single request from a Sub-Hub.
pub async fn dispatch_meta_request(
    meta_hub: &Arc<Mutex<MetaHub>>,
    method: &str,
    params: Value,
    from_hub: String,
) -> Result<Value, String> {
    match method {
        m if m == methods::META_ROUTE.name => handle_meta_route(meta_hub, params, &from_hub).await,
        m if m == methods::META_RESOLVE.name => handle_meta_resolve(meta_hub, params).await,
        m if m == methods::META_SPAWN.name => handle_meta_spawn(meta_hub, params).await,
        m if m == methods::META_LIST_HUBS.name => handle_meta_list_hubs(meta_hub).await,
        m if m == methods::META_TOPOLOGY.name => handle_meta_topology(meta_hub).await,
        // Permission/question relay from Sub-Hub agents → MetaHub UI clients
        m if m == methods::AGENT_PERMISSION.name || m == methods::AGENT_QUESTION.name => {
            handle_permission_relay(meta_hub, method, params, &from_hub).await
        }
        _ => Err(format!("unknown meta method: {method}")),
    }
}

/// Route envelope to correct Sub-Hub. Detects self-routing.
async fn handle_meta_route(
    meta_hub: &Arc<Mutex<MetaHub>>,
    params: Value,
    from_hub: &str,
) -> Result<Value, String> {
    let envelope: loopal_protocol::Envelope =
        serde_json::from_value(params).map_err(|e| format!("invalid envelope: {e}"))?;

    // Self-routing detection: if target explicitly names the originating hub, reject.
    let addr = loopal_protocol::QualifiedAddress::parse(&envelope.target);
    if addr.hub.as_deref() == Some(from_hub) {
        return Err(format!(
            "self-routing detected: target '{}' is on originating hub '{from_hub}', route locally",
            envelope.target
        ));
    }

    let mut mh = meta_hub.lock().await;
    let candidates: Vec<(String, Arc<loopal_ipc::connection::Connection>)> = mh
        .registry
        .alive_hubs()
        .into_iter()
        .map(|(name, conn)| (name.to_string(), conn.clone()))
        .collect();
    let refs: Vec<(&str, &Arc<loopal_ipc::connection::Connection>)> =
        candidates.iter().map(|(n, c)| (n.as_str(), c)).collect();

    mh.router.route(&envelope, &refs).await?;
    Ok(json!({"ok": true}))
}

/// Resolve whether an agent exists on any Sub-Hub.
///
/// Single lock acquisition — candidates snapshot and resolution happen together.
async fn handle_meta_resolve(
    meta_hub: &Arc<Mutex<MetaHub>>,
    params: Value,
) -> Result<Value, String> {
    let agent_name = params["agent_name"]
        .as_str()
        .ok_or("missing 'agent_name'")?;

    let mut mh = meta_hub.lock().await;
    let candidates: Vec<(String, Arc<loopal_ipc::connection::Connection>)> = mh
        .registry
        .alive_hubs()
        .into_iter()
        .map(|(name, conn)| (name.to_string(), conn.clone()))
        .collect();
    let refs: Vec<(&str, &Arc<loopal_ipc::connection::Connection>)> =
        candidates.iter().map(|(n, c)| (n.as_str(), c)).collect();

    match mh.router.resolve_agent(agent_name, &refs).await {
        Some(hub_name) => Ok(json!({"found": true, "hub": hub_name})),
        None => Ok(json!({"found": false})),
    }
}

/// Delegate agent spawn to a specific Sub-Hub.
async fn handle_meta_spawn(meta_hub: &Arc<Mutex<MetaHub>>, params: Value) -> Result<Value, String> {
    let target_hub = params["target_hub"]
        .as_str()
        .ok_or("missing 'target_hub'")?
        .to_string();

    let conn = {
        let mh = meta_hub.lock().await;
        mh.registry
            .connection(&target_hub)
            .ok_or_else(|| format!("hub '{target_hub}' not connected"))?
    };

    // Forward as hub/spawn_agent (strip target_hub field)
    let mut spawn_params = params.clone();
    if let Some(obj) = spawn_params.as_object_mut() {
        obj.remove("target_hub");
    }

    conn.send_request(methods::HUB_SPAWN_AGENT.name, spawn_params)
        .await
        .map_err(|e| format!("spawn on '{target_hub}' failed: {e}"))
}

/// List all connected Sub-Hubs.
async fn handle_meta_list_hubs(meta_hub: &Arc<Mutex<MetaHub>>) -> Result<Value, String> {
    let mh = meta_hub.lock().await;
    let hubs: Vec<Value> = mh
        .registry
        .snapshot()
        .into_iter()
        .map(|i| {
            json!({"name": i.name, "status": format!("{:?}", i.status),
               "agent_count": i.agent_count, "capabilities": i.capabilities})
        })
        .collect();
    Ok(json!({"hubs": hubs}))
}

/// Global topology: aggregate topology from all Sub-Hubs.
async fn handle_meta_topology(meta_hub: &Arc<Mutex<MetaHub>>) -> Result<Value, String> {
    let candidates = {
        let mh = meta_hub.lock().await;
        mh.registry
            .alive_hubs()
            .into_iter()
            .map(|(name, conn)| (name.to_string(), conn.clone()))
            .collect::<Vec<_>>()
    };

    let mut hubs = Vec::new();
    let timeout = std::time::Duration::from_secs(10);
    for (name, conn) in &candidates {
        let topology = match tokio::time::timeout(
            timeout,
            conn.send_request(methods::HUB_TOPOLOGY.name, json!({})),
        )
        .await
        {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => json!({"error": "unreachable"}),
            Err(_) => json!({"error": "timeout"}),
        };
        hubs.push(json!({"hub": name, "topology": topology}));
    }
    Ok(json!({"hubs": hubs}))
}

/// Relay permission/question request to MetaHub's own UI clients.
async fn handle_permission_relay(
    meta_hub: &Arc<Mutex<MetaHub>>,
    method: &str,
    params: Value,
    from_hub: &str,
) -> Result<Value, String> {
    let ui_conns = {
        let mh = meta_hub.lock().await;
        mh.ui.get_client_connections()
    };

    if ui_conns.is_empty() {
        tracing::warn!(hub = %from_hub, %method, "no MetaHub UI clients, denying");
        return Ok(json!({"allow": false}));
    }

    // Relay to first available UI client with timeout
    if let Some((client_name, conn)) = ui_conns.first() {
        match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            conn.send_request(method, params),
        )
        .await
        {
            Ok(Ok(resp)) => {
                tracing::info!(hub = %from_hub, client = %client_name, %method, "relay succeeded");
                return Ok(resp);
            }
            Ok(Err(e)) => {
                tracing::warn!(hub = %from_hub, client = %client_name, error = %e, "relay failed");
            }
            Err(_) => {
                tracing::warn!(hub = %from_hub, client = %client_name, "relay timed out (30s)");
            }
        }
    }

    Ok(json!({"allow": false}))
}
