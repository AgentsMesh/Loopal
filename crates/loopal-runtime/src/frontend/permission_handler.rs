use async_trait::async_trait;
use loopal_tool_api::PermissionDecision;

/// Permission handler trait for agent permission decisions.
///
/// `UnifiedFrontend` delegates `request_permission` to this trait,
/// enabling pluggable strategies: auto-deny for sub-agents, relay forwarding
/// for root agents, or custom logic.
#[async_trait]
pub trait PermissionHandler: Send + Sync {
    async fn decide(&self, id: &str, name: &str, input: &serde_json::Value) -> PermissionDecision;
}

/// Default handler: deny all permission requests (no human in the loop).
pub struct AutoDenyHandler;

#[async_trait]
impl PermissionHandler for AutoDenyHandler {
    async fn decide(
        &self,
        _id: &str,
        _name: &str,
        _input: &serde_json::Value,
    ) -> PermissionDecision {
        PermissionDecision::Deny
    }
}
