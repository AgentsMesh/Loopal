//! TUI relay — proxies permission/question requests to the TUI client.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tracing::warn;

use loopal_ipc::connection::Connection;

use crate::hub::AgentHub;

/// Timeout for TUI relay — auto-deny if TUI doesn't respond within this window.
const TUI_RELAY_TIMEOUT: Duration = Duration::from_secs(30);

/// Relay a permission/question request from an agent to the TUI.
pub(crate) async fn relay_to_tui(
    hub: &Arc<Mutex<AgentHub>>,
    agent_conn: &Arc<Connection>,
    request_id: i64,
    method: &str,
    params: serde_json::Value,
    agent_name: &str,
) {
    let tui_conn = {
        let h = hub.lock().await;
        h.get_agent_connection("_tui")
    };

    let Some(tui) = tui_conn else {
        warn!(agent = %agent_name, %method, "no TUI connected, denying");
        let _ = agent_conn
            .respond(request_id, serde_json::json!({"allow": false}))
            .await;
        return;
    };

    let result = tokio::time::timeout(TUI_RELAY_TIMEOUT, tui.send_request(method, params)).await;
    match result {
        Ok(Ok(response)) => {
            let _ = agent_conn.respond(request_id, response).await;
            return;
        }
        Ok(Err(e)) => warn!(agent = %agent_name, %method, error = %e, "TUI relay failed"),
        Err(_) => warn!(agent = %agent_name, %method, "TUI relay timed out"),
    }
    let _ = agent_conn
        .respond(request_id, serde_json::json!({"allow": false}))
        .await;
}
