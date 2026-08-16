use async_trait::async_trait;
use loopal_protocol::{PermissionIntentRequest, PermissionReceipt};
use loopal_tool_api::PermissionDecision;

#[derive(Debug, Clone)]
pub struct PermissionOutcome {
    pub decision: PermissionDecision,
    pub reason: String,
    pub duration_ms: u64,
    /// Hub-issued authorization for the exact approved effect, when the
    /// frontend is backed by a Hub permission lease.
    pub receipt: Option<PermissionReceipt>,
}

impl PermissionOutcome {
    pub fn allow() -> Self {
        Self {
            decision: PermissionDecision::Allow,
            reason: String::new(),
            duration_ms: 0,
            receipt: None,
        }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            decision: PermissionDecision::Deny,
            reason: reason.into(),
            duration_ms: 0,
            receipt: None,
        }
    }
}

#[async_trait]
pub trait PermissionHandler: Send + Sync {
    async fn decide(&self, request: &PermissionIntentRequest) -> PermissionOutcome;
}
