//! ACP lifecycle handlers: initialize, authenticate.

use serde_json::Value;
use tracing::info;

use crate::adapter::AcpAdapter;
use crate::types::make_init_response;

impl AcpAdapter {
    /// Handle `initialize` — return agent capabilities and info. Advertises
    /// the AgentsMesh `controlRequest` extension so the runner routes Loopal
    /// control-panel actions (bg-task kill / cron delete) via
    /// `session/control_request`.
    pub(crate) async fn handle_initialize(&self, id: i64, _params: Value) {
        let mut result = serde_json::to_value(make_init_response()).unwrap_or_default();
        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "agentsmeshExtensions".into(),
                serde_json::json!({ "controlRequest": true }),
            );
        }
        self.acp_out.respond(id, result).await;
        info!("ACP initialized");
    }

    /// Handle `authenticate` — Loopal uses IDE's auth context, no agent-side
    /// validation needed. Always returns success.
    pub(crate) async fn handle_authenticate(&self, id: i64, _params: Value) {
        self.acp_out.respond(id, serde_json::json!({})).await;
    }
}
