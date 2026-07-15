use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use loopal_ipc::protocol::methods;

use crate::meta_hub::MetaHub;

const SPAWN_FORWARD_TIMEOUT: Duration = Duration::from_secs(30);

/// Dispatch a single request from a Sub-Hub.
pub async fn dispatch_meta_request(
    meta_hub: &Arc<Mutex<MetaHub>>,
    method: &str,
    params: Value,
    from_hub: String,
) -> Result<Value, String> {
    match method {
        m if m == methods::META_ROUTE.name => handle_meta_route(meta_hub, params, &from_hub).await,
        m if m == methods::META_SPAWN.name => handle_meta_spawn(meta_hub, params).await,
        m if m == methods::META_LIST_HUBS.name => handle_meta_list_hubs(meta_hub).await,
        m if m == methods::META_TOPOLOGY.name => handle_meta_topology(meta_hub).await,
        m if m == methods::META_REMOTE_RELAY.name => {
            crate::remote_relay::forward(meta_hub, params).await
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

    // Self-routing detection: the next hop in target equals the originating hub.
    if envelope.target.next_hop() == Some(from_hub) {
        return Err(format!(
            "self-routing detected: target '{}' next-hop is originating hub '{from_hub}', \
             route locally instead",
            envelope.target
        ));
    }

    let (router, candidates) = {
        let mh = meta_hub.lock().await;
        let candidates = mh
            .registry
            .alive_hubs()
            .into_iter()
            .map(|(name, conn)| (name.to_string(), conn.clone()))
            .collect::<Vec<_>>();
        (mh.router, candidates)
    };
    let refs: Vec<(
        &str,
        &Arc<loopal_ipc::connection::Connection<loopal_ipc::connection::Listening>>,
    )> = candidates.iter().map(|(n, c)| (n.as_str(), c)).collect();

    router.route(&envelope, &refs).await?;
    Ok(json!({"ok": true}))
}

/// Delegate agent spawn to a specific Sub-Hub.
///
/// Cross-hub spawn cannot share filesystem state with the originating hub,
/// so `cwd` / `fork_context` / `resume` are forbidden in the payload —
/// receiver Hub uses its local `default_cwd`.
async fn handle_meta_spawn(meta_hub: &Arc<Mutex<MetaHub>>, params: Value) -> Result<Value, String> {
    let target_hub = params["target_hub"]
        .as_str()
        .ok_or("missing 'target_hub'")?
        .to_string();

    loopal_ipc::cross_hub::validate_spawn_payload(&params)?;

    let conn = {
        let mh = meta_hub.lock().await;
        mh.registry
            .connection(&target_hub)
            .ok_or_else(|| format!("hub '{target_hub}' not connected"))?
    };

    // Forward as hub/spawn_remote_agent (strip target_hub field). The new
    // method signals to the receiver that filesystem-coupled fields must be
    // ignored / rejected; receiver uses its own default_cwd.
    let mut spawn_params = params.clone();
    if let Some(obj) = spawn_params.as_object_mut() {
        obj.remove("target_hub");
    }

    tokio::time::timeout(
        SPAWN_FORWARD_TIMEOUT,
        conn.send_request(methods::HUB_SPAWN_REMOTE_AGENT.name, spawn_params),
    )
    .await
    .map_err(|_| format!("spawn on '{target_hub}' timed out"))?
    .map_err(|e| format!("spawn on '{target_hub}' failed: {e}"))
}

/// List all connected Sub-Hubs.
async fn handle_meta_list_hubs(meta_hub: &Arc<Mutex<MetaHub>>) -> Result<Value, String> {
    let mh = meta_hub.lock().await;
    let mut hubs: Vec<Value> = mh
        .registry
        .snapshot()
        .into_iter()
        .map(|i| {
            json!({"name": i.name, "status": format!("{:?}", i.status),
               "agent_count": i.agent_count, "capabilities": i.capabilities})
        })
        .collect();
    hubs.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
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

    let mut pending = tokio::task::JoinSet::new();
    for (name, conn) in candidates {
        pending.spawn(async move {
            let topology = match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                conn.send_request(methods::HUB_TOPOLOGY.name, json!({})),
            )
            .await
            {
                Ok(Ok(value)) => without_shadows(value),
                Ok(Err(_)) => json!({"error": "unreachable"}),
                Err(_) => json!({"error": "timeout"}),
            };
            json!({"hub": name, "topology": topology})
        });
    }
    let mut hubs = Vec::new();
    while let Some(result) = pending.join_next().await {
        if let Ok(value) = result {
            hubs.push(value);
        }
    }
    hubs.sort_by(|left, right| left["hub"].as_str().cmp(&right["hub"].as_str()));
    Ok(json!({"hubs": hubs}))
}

fn without_shadows(mut topology: Value) -> Value {
    let Some(agents) = topology.get_mut("agents").and_then(Value::as_array_mut) else {
        return topology;
    };
    let shadows = agents
        .iter()
        .filter(|agent| agent["shadow"].as_bool() == Some(true))
        .filter_map(|agent| agent["name"].as_str().map(String::from))
        .collect::<std::collections::HashSet<_>>();
    agents.retain(|agent| agent["shadow"].as_bool() != Some(true));
    for agent in agents {
        if let Some(children) = agent.get_mut("children").and_then(Value::as_array_mut) {
            children.retain(|child| child.as_str().is_none_or(|name| !shadows.contains(name)));
        }
    }
    topology
}
