//! Hub uplink — optional connection to a parent MetaHub.
//!
//! When present, enables cross-hub communication by forwarding requests
//! that cannot be handled locally (unknown agents, remote spawn targets,
//! permission relay with no local UI).
//!
//! When absent (`Hub.uplink == None`), the Hub operates in standalone mode
//! with identical behavior to the pre-MetaHub architecture.

use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;
use serde_json::{Value, json};

/// Connection from a Hub to its parent MetaHub.
///
/// This is the **sole injection point** for cross-hub communication.
/// All existing Hub logic remains unchanged — the uplink is only
/// consulted when local handling fails.
pub struct HubUplink {
    /// TCP connection to the MetaHub.
    conn: Arc<Connection>,
    /// This Hub's registered name on the MetaHub.
    hub_name: String,
}

impl HubUplink {
    /// Create an uplink from an already-established connection.
    ///
    /// The caller is responsible for TCP connect + `meta/register` handshake.
    /// This constructor just wraps the authenticated connection.
    pub fn new(conn: Arc<Connection>, hub_name: String) -> Self {
        Self { conn, hub_name }
    }

    /// This Hub's name as registered on the MetaHub.
    pub fn hub_name(&self) -> &str {
        &self.hub_name
    }

    /// The underlying connection (for advanced use / event subscription).
    pub fn connection(&self) -> &Arc<Connection> {
        &self.conn
    }

    /// Route an envelope to a remote agent via MetaHub.
    ///
    /// Applies SNAT before forwarding: stamps this hub's name onto the
    /// envelope source so the receiver sees the full return path.
    pub async fn route(&self, envelope: &Envelope) -> Result<(), String> {
        let mut env = envelope.clone();
        env.apply_snat(&self.hub_name);
        let params = serde_json::to_value(&env).map_err(|e| format!("serialize envelope: {e}"))?;
        let resp = self
            .conn
            .send_request(methods::META_ROUTE.name, params)
            .await
            .map_err(|e| format!("meta/route failed: {e}"))?;
        // JSON-RPC errors come back as Ok(json with code/message)
        if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
            return Err(format!("meta/route error: {msg}"));
        }
        Ok(())
    }

    /// Delegate agent spawn to a remote Hub via MetaHub.
    pub async fn spawn_agent(&self, params: Value) -> Result<Value, String> {
        let resp = self
            .conn
            .send_request(methods::META_SPAWN.name, params)
            .await
            .map_err(|e| format!("meta/spawn failed: {e}"))?;
        if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
            return Err(format!("meta/spawn error: {msg}"));
        }
        Ok(resp)
    }

    /// Relay a permission/question request to MetaHub's UI clients.
    pub async fn relay_permission(&self, method: &str, params: Value) -> Result<Value, String> {
        let resp = self
            .conn
            .send_request(method, params)
            .await
            .map_err(|e| format!("uplink relay {method} failed: {e}"))?;
        if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
            return Err(format!("uplink relay error: {msg}"));
        }
        Ok(resp)
    }

    /// Send heartbeat to MetaHub with current agent count.
    pub async fn heartbeat(&self, agent_count: usize) -> Result<(), String> {
        self.conn
            .send_notification(
                methods::META_HEARTBEAT.name,
                json!({
                    "hub_name": self.hub_name,
                    "agent_count": agent_count,
                }),
            )
            .await
            .map_err(|e| format!("meta/heartbeat failed: {e}"))
    }
}

/// Process reverse requests from MetaHub (agent/message, hub/*).
///
/// Shared implementation used by both production bootstrap and integration tests.
/// Runs until the connection closes.
pub async fn handle_reverse_requests(
    hub: Arc<Mutex<crate::hub::Hub>>,
    conn: Arc<Connection>,
    mut rx: mpsc::Receiver<Incoming>,
    hub_name: String,
) {
    tracing::info!(hub = %hub_name, "MetaHub reverse handler started");
    while let Some(msg) = rx.recv().await {
        match msg {
            Incoming::Request { id, method, params } => {
                if method == methods::AGENT_MESSAGE.name {
                    let ok = if let Ok(env) =
                        serde_json::from_value::<loopal_protocol::Envelope>(params)
                    {
                        // Remote agent completions arrive with the agent-result
                        // marker in content. Detect by content (not source tag)
                        // so it works with the typed Agent source after SNAT.
                        if let Some(child) = extract_agent_result_name(&env) {
                            let output = env.content.text.clone();
                            let mut h = hub.lock().await;
                            h.registry.emit_agent_finished(&child, Some(output));
                            h.registry.unregister_connection(&child);
                        }
                        // Defense in depth: target should be local at this point
                        // (MetaHub router consumed the next-hop hub via DNAT).
                        debug_assert!(
                            env.target.is_local(),
                            "target should be local after MetaHub DNAT, got {:?}",
                            env.target
                        );
                        hub.lock().await.registry.route_message(&env).await.is_ok()
                    } else {
                        false
                    };
                    let _ = conn.respond(id, json!({"ok": ok})).await;
                } else {
                    match crate::dispatch::dispatch_hub_request(
                        &hub,
                        &method,
                        params,
                        format!("meta:{hub_name}"),
                    )
                    .await
                    {
                        Ok(r) => {
                            let _ = conn.respond(id, r).await;
                        }
                        Err(e) => {
                            let _ = conn
                                .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &e)
                                .await;
                        }
                    }
                }
            }
            Incoming::Notification { method, params } => {
                if method == methods::AGENT_MESSAGE.name
                    && let Ok(env) = serde_json::from_value::<loopal_protocol::Envelope>(params)
                {
                    debug_assert!(
                        env.target.is_local(),
                        "notification target should be local after DNAT, got {:?}",
                        env.target
                    );
                    let _ = hub.lock().await.registry.route_message(&env).await;
                }
            }
        }
    }
    tracing::warn!(hub = %hub_name, "MetaHub reverse handler ended");
}

/// Extract child agent name from `<agent-result name="...">` envelope.
fn extract_agent_result_name(env: &loopal_protocol::Envelope) -> Option<String> {
    let text = &env.content.text;
    let start = text.find("<agent-result name=\"")? + 20;
    let end = text[start..].find('"')? + start;
    Some(text[start..end].to_string())
}
