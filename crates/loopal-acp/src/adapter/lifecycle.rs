//! ACP lifecycle handlers: initialize, authenticate.

use serde_json::Value;
use tracing::info;

use crate::adapter::AcpAdapter;
use crate::types::make_init_response;

/// `initialize` result + AgentsMesh extensions: `controlRequest` (runner routes
/// control-panel actions via `session/control_request`) and `permissionModes`
/// (AgentsMesh selector renders loopal's modes, not Claude Code's).
/// `permissionModes` must match `loopal_tool_api::PermissionMode` (SSOT).
fn init_response_with_extensions() -> Value {
    let mut result = serde_json::to_value(make_init_response()).unwrap_or_default();
    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "agentsmeshExtensions".into(),
            serde_json::json!({
                "controlRequest": true,
                "permissionModes": ["bypass", "ask_dangerous", "ask_any_write"],
            }),
        );
    }
    result
}

impl AcpAdapter {
    pub(crate) async fn handle_initialize(&self, id: i64, _params: Value) {
        self.acp_out
            .respond(id, init_response_with_extensions())
            .await;
        info!("ACP initialized");
    }

    /// Handle `authenticate` — Loopal uses IDE's auth context, no agent-side
    /// validation needed. Always returns success.
    pub(crate) async fn handle_authenticate(&self, id: i64, _params: Value) {
        self.acp_out.respond(id, serde_json::json!({})).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_response_advertises_control_and_permission_modes() {
        let resp = init_response_with_extensions();
        let ext = &resp["agentsmeshExtensions"];
        assert_eq!(ext["controlRequest"], serde_json::json!(true));
        assert_eq!(
            ext["permissionModes"],
            serde_json::json!(["bypass", "ask_dangerous", "ask_any_write"])
        );
    }
}
