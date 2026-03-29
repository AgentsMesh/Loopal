//! ACP lifecycle handlers: initialize, authenticate.

use serde_json::Value;
use tracing::info;

use crate::adapter::AcpAdapter;
use crate::types::make_init_response;

impl AcpAdapter {
    /// Handle `initialize` — return agent capabilities and info.
    pub(crate) async fn handle_initialize(&self, id: i64, _params: Value) {
        let result = make_init_response();
        self.acp_out
            .respond(id, serde_json::to_value(result).unwrap_or_default())
            .await;
        info!("ACP initialized");
    }

    /// Handle `authenticate` — Loopal uses IDE's auth context, no agent-side
    /// validation needed. Always returns success.
    pub(crate) async fn handle_authenticate(&self, id: i64, _params: Value) {
        self.acp_out.respond(id, serde_json::json!({})).await;
    }
}
