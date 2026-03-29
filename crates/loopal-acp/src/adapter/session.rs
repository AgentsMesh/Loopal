//! ACP session handlers: new, list, close.

use serde_json::Value;
use tracing::info;

use crate::adapter::AcpAdapter;
use crate::jsonrpc;
use crate::types::make_new_session_response;

impl AcpAdapter {
    /// Handle `session/new` — assign session ID, drain bootstrap events.
    pub(crate) async fn handle_new_session(&self, id: i64, _params: Value) {
        let sid = uuid::Uuid::new_v4().to_string();
        *self.session_id.lock().await = Some(sid.clone());
        self.acp_out
            .respond(
                id,
                serde_json::to_value(make_new_session_response(sid)).unwrap_or_default(),
            )
            .await;
        info!("ACP session created");
        self.drain_bootstrap_events().await;
    }

    /// Handle `session/list` → HubClient.list_agents().
    pub(crate) async fn handle_list_sessions(&self, id: i64) {
        match self.client.list_agents().await {
            Ok(result) => {
                self.acp_out
                    .respond(id, serde_json::json!({"sessions": result}))
                    .await;
            }
            Err(e) => {
                self.acp_out
                    .respond_error(id, jsonrpc::INTERNAL_ERROR, &e)
                    .await;
            }
        }
    }

    /// Handle `session/close` → HubClient.shutdown_agent().
    pub(crate) async fn handle_close(&self, id: i64, params: Value) {
        let requested_sid = params["sessionId"].as_str().unwrap_or("");
        let current_sid = self.session_id.lock().await.clone();

        if let Some(ref sid) = current_sid {
            if !requested_sid.is_empty() && requested_sid != sid.as_str() {
                self.acp_out
                    .respond_error(id, jsonrpc::INVALID_REQUEST, "session mismatch")
                    .await;
                return;
            }
        }

        self.client.shutdown_agent().await;
        *self.session_id.lock().await = None;
        self.acp_out.respond(id, serde_json::json!({})).await;
        info!("ACP session closed");
    }
}
