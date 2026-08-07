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

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use serde_json::json;

const REVERSE_RESPONSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

#[path = "uplink/reverse_route.rs"]
pub(crate) mod reverse_route;

/// Capability token proving a request came from this Hub's authenticated
/// reverse MetaHub transport. Its private field prevents string identities
/// from manufacturing the authority.
pub(crate) struct TrustedMetaHubContext {
    connection: Arc<Connection<Listening>>,
}

impl TrustedMetaHubContext {
    pub(crate) fn matches(&self, uplink: &HubUplink) -> bool {
        Arc::ptr_eq(&self.connection, uplink.connection())
    }
}

/// Connection from a Hub to its parent MetaHub.
///
/// This is the **sole injection point** for cross-hub communication.
/// All existing Hub logic remains unchanged — the uplink is only
/// consulted when local handling fails.
pub struct HubUplink {
    /// TCP connection to the MetaHub.
    conn: Arc<Connection<Listening>>,
    /// This Hub's registered name on the MetaHub.
    hub_name: String,
    meta_address: Option<String>,
}

impl HubUplink {
    /// Create an uplink from an already-established connection.
    ///
    /// The caller is responsible for TCP connect + `meta/register` handshake.
    /// This constructor just wraps the authenticated connection.
    pub fn new(conn: Arc<Connection<Listening>>, hub_name: String) -> Self {
        Self {
            conn,
            hub_name,
            meta_address: None,
        }
    }

    pub fn with_address(
        conn: Arc<Connection<Listening>>,
        hub_name: String,
        meta_address: String,
    ) -> Self {
        Self {
            conn,
            hub_name,
            meta_address: Some(meta_address),
        }
    }

    /// This Hub's name as registered on the MetaHub.
    pub fn hub_name(&self) -> &str {
        &self.hub_name
    }

    pub fn meta_address(&self) -> Option<&str> {
        self.meta_address.as_deref()
    }

    /// The underlying connection (for advanced use / event subscription).
    pub fn connection(&self) -> &Arc<Connection<Listening>> {
        &self.conn
    }

    /// Send heartbeat to MetaHub with current agent count.
    pub async fn heartbeat(&self, agent_count: usize) -> Result<(), String> {
        self.conn
            .send_request(
                methods::META_HEARTBEAT.name,
                json!({
                    "hub_name": self.hub_name,
                    "agent_count": agent_count,
                }),
            )
            .await
            .map(|_| ())
            .map_err(|e| format!("meta/heartbeat failed: {e}"))
    }
}

/// Process reverse requests from MetaHub (agent/message, hub/*).
///
/// Shared implementation used by both production bootstrap and integration tests.
/// Runs until the connection closes.
pub async fn handle_reverse_requests(
    hub: Arc<Mutex<crate::hub::Hub>>,
    conn: Arc<Connection<Listening>>,
    mut rx: mpsc::Receiver<Incoming>,
    hub_name: String,
) {
    tracing::info!(hub = %hub_name, "MetaHub reverse handler started");
    let dispatcher = Arc::new(crate::dispatch::build_hub_dispatcher(hub.clone()));
    while let Some(msg) = rx.recv().await {
        match msg {
            Incoming::Request { id, method, params } => {
                if method == methods::AGENT_MESSAGE.name {
                    let ok = if let Ok(env) =
                        serde_json::from_value::<loopal_protocol::Envelope>(params)
                    {
                        // Remote agent completions arrive as a typed AgentResult
                        // source (set by the origin hub, survives SNAT). The
                        // child name is the bare agent segment.
                        let is_agent_result = matches!(
                            &env.source,
                            loopal_protocol::MessageSource::AgentResult { .. }
                        );
                        let parent_generation =
                            if let loopal_protocol::MessageSource::AgentResult { child } =
                                &env.source
                            {
                                // Pre-metadata envelopes are successful legacy
                                // completions; new envelopes preserve reason+result.
                                let completion =
                                    env.agent_completion.clone().unwrap_or_else(|| {
                                        loopal_protocol::AgentCompletion::goal(Some(
                                            env.content.text.clone(),
                                        ))
                                    });
                                let drop_completion = {
                                    let h = hub.lock().await;
                                    !h.is_active_uplink_connection(&conn)
                                        || h.should_drop_quarantined_completion(&child.agent, &conn)
                                };
                                if drop_completion {
                                    tracing::warn!(agent = %child.agent, "dropping completion from stale/quarantined uplink lease");
                                    None
                                } else if crate::finish::cache_cross_hub_completion_if_spawning(
                                    &hub,
                                    &child.agent,
                                    completion.clone(),
                                    env.clone(),
                                )
                                .await
                                {
                                    // The spawn coordinator drains this typed
                                    // completion after SubAgentSpawned admission.
                                    None
                                } else {
                                    crate::finish::record_cross_hub_completion_from_uplink(
                                        &hub,
                                        &child.agent,
                                        completion,
                                        &conn,
                                    )
                                    .await
                                    .local_parent_generation()
                                }
                            } else {
                                None
                            };
                        // Defense in depth: target should be local at this point
                        // (MetaHub router consumed the next-hop hub via DNAT).
                        debug_assert!(
                            env.target.is_local(),
                            "target should be local after MetaHub DNAT, got {:?}",
                            env.target
                        );
                        if let Some(parent_generation) = parent_generation {
                            reverse_route::deliver_for_generation(&hub, &env, parent_generation)
                                .await
                        } else if !is_agent_result {
                            reverse_route::deliver(&hub, &env).await
                        } else {
                            // The completion was consumed by a foreground
                            // wait_agent call (or was a duplicate). Its
                            // transport still succeeded even though it is not
                            // a new parent input.
                            true
                        }
                    } else {
                        false
                    };
                    if !respond_reverse(&conn, id, Ok(json!({"ok": ok}))).await {
                        break;
                    }
                } else {
                    let result = if method == methods::HUB_REMOTE_RELAY.name {
                        let trusted = TrustedMetaHubContext {
                            connection: conn.clone(),
                        };
                        crate::remote_relay::handle(&hub, params, &trusted).await
                    } else {
                        crate::dispatch::dispatch_hub_request_with(
                            &dispatcher,
                            &method,
                            params,
                            format!("uplink:{hub_name}"),
                        )
                        .await
                    };
                    if !respond_reverse(&conn, id, result).await {
                        break;
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
                    let _ = reverse_route::deliver(&hub, &env).await;
                } else if method == methods::REQUEST_CANCEL.name {
                    // Requests are handled serially above. By the time their
                    // cancellation reaches this loop, the handler has already
                    // completed; interaction state has its own token cleanup.
                    tracing::debug!("late reverse request cancellation observed");
                }
            }
        }
    }
    tracing::warn!(hub = %hub_name, "MetaHub reverse handler ended");
}

async fn respond_reverse(
    conn: &Arc<Connection<Listening>>,
    id: i64,
    result: Result<serde_json::Value, String>,
) -> bool {
    let response = async {
        match result {
            Ok(value) => conn.respond(id, value).await,
            Err(error) => {
                conn.respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &error)
                    .await
            }
        }
    };
    match tokio::time::timeout(REVERSE_RESPONSE_TIMEOUT, response).await {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            tracing::warn!(%error, "MetaHub reverse response failed; closing uplink");
            close_reverse(conn).await;
            false
        }
        Err(_) => {
            tracing::warn!("MetaHub reverse response timed out; closing uplink");
            close_reverse(conn).await;
            false
        }
    }
}

async fn close_reverse(conn: &Connection<Listening>) {
    let _ = tokio::time::timeout(REVERSE_RESPONSE_TIMEOUT, conn.close()).await;
}
