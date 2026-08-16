use async_trait::async_trait;
use loopal_protocol::{PermissionDecisionAuditRequest, ProtectedEffectAuditRequest};

#[async_trait]
pub trait ProtectedEffectAudit: Send + Sync {
    async fn record(&self, request: &ProtectedEffectAuditRequest) -> loopal_error::Result<()>;
    async fn record_permission_decision(
        &self,
        request: &PermissionDecisionAuditRequest,
    ) -> loopal_error::Result<()>;
}

pub struct NoopProtectedEffectAudit;

#[async_trait]
impl ProtectedEffectAudit for NoopProtectedEffectAudit {
    async fn record(&self, _request: &ProtectedEffectAuditRequest) -> loopal_error::Result<()> {
        Ok(())
    }

    async fn record_permission_decision(
        &self,
        _request: &PermissionDecisionAuditRequest,
    ) -> loopal_error::Result<()> {
        Ok(())
    }
}
