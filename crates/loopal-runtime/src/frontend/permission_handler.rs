use async_trait::async_trait;
use loopal_tool_api::PermissionDecision;

#[derive(Debug, Clone)]
pub struct PermissionOutcome {
    pub decision: PermissionDecision,
    pub reason: String,
    pub duration_ms: u64,
}

impl PermissionOutcome {
    pub fn allow() -> Self {
        Self {
            decision: PermissionDecision::Allow,
            reason: String::new(),
            duration_ms: 0,
        }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            decision: PermissionDecision::Deny,
            reason: reason.into(),
            duration_ms: 0,
        }
    }
}

#[async_trait]
pub trait PermissionHandler: Send + Sync {
    async fn decide(&self, id: &str, name: &str, input: &serde_json::Value) -> PermissionOutcome;
}
