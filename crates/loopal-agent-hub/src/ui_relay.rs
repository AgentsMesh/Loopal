//! UI relay — races permission/question requests across all UI clients.
//!
//! When an agent requests permission, the Hub broadcasts the request to ALL
//! registered UI clients (TUI, ACP, etc.) concurrently. The first response
//! wins — subsequent responses are discarded.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tracing::{info, warn};

use loopal_ipc::connection::Connection;

use crate::hub::Hub;

/// Timeout for UI relay — auto-deny if no UI client responds within this window.
const UI_RELAY_TIMEOUT: Duration = Duration::from_secs(30);

/// Relay a permission/question request from an agent to all UI clients (race model).
///
/// Sends the request concurrently to every registered UI client. The first
/// response wins; the agent receives that response immediately.
pub(crate) async fn relay_to_ui_clients(
    hub: &Arc<Mutex<Hub>>,
    agent_conn: &Arc<Connection>,
    request_id: i64,
    method: &str,
    params: serde_json::Value,
    agent_name: &str,
) {
    let ui_conns = {
        let h = hub.lock().await;
        h.ui.get_client_connections()
    };

    if ui_conns.is_empty() {
        warn!(agent = %agent_name, %method, "no UI clients connected, denying");
        let _ = agent_conn
            .respond(request_id, serde_json::json!({"allow": false}))
            .await;
        return;
    }

    // Single UI client: direct relay (no race overhead)
    if ui_conns.len() == 1 {
        let (name, conn) = &ui_conns[0];
        let response = relay_single(conn, method, &params, name, agent_name).await;
        let _ = agent_conn.respond(request_id, response).await;
        return;
    }

    // Multiple UI clients: race — first response wins
    let response = race_relay(&ui_conns, method, &params, agent_name).await;
    let _ = agent_conn.respond(request_id, response).await;
}

/// Relay to a single UI client with timeout.
async fn relay_single(
    conn: &Arc<Connection>,
    method: &str,
    params: &serde_json::Value,
    client_name: &str,
    agent_name: &str,
) -> serde_json::Value {
    match tokio::time::timeout(UI_RELAY_TIMEOUT, conn.send_request(method, params.clone())).await {
        Ok(Ok(response)) => {
            info!(agent = %agent_name, client = %client_name, %method, "UI relay succeeded");
            response
        }
        Ok(Err(e)) => {
            warn!(agent = %agent_name, client = %client_name, error = %e, "UI relay failed");
            serde_json::json!({"allow": false})
        }
        Err(_) => {
            warn!(agent = %agent_name, client = %client_name, "UI relay timed out");
            serde_json::json!({"allow": false})
        }
    }
}

/// Race relay across multiple UI clients. First response wins.
async fn race_relay(
    ui_conns: &[(String, Arc<Connection>)],
    method: &str,
    params: &serde_json::Value,
    agent_name: &str,
) -> serde_json::Value {
    use tokio::sync::oneshot;

    let (tx, rx) = oneshot::channel();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

    for (name, conn) in ui_conns {
        let conn = conn.clone();
        let method = method.to_string();
        let params = params.clone();
        let name = name.clone();
        let agent_name = agent_name.to_string();
        let tx = tx.clone();

        tokio::spawn(async move {
            let result = relay_single(&conn, &method, &params, &name, &agent_name).await;
            if let Some(sender) = tx.lock().await.take() {
                let _ = sender.send(result);
            }
        });
    }

    match tokio::time::timeout(UI_RELAY_TIMEOUT, rx).await {
        Ok(Ok(response)) => response,
        _ => {
            warn!(agent = %agent_name, "all UI relays failed or timed out");
            serde_json::json!({"allow": false})
        }
    }
}
