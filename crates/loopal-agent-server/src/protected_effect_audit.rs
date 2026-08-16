use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    PermissionDecisionAuditRequest, PermissionDecisionAuditResponse, ProtectedEffectAuditRequest,
    ProtectedEffectAuditResponse,
};
use loopal_tool_api::ProtectedEffectAudit;

const AUDIT_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(test)]
#[path = "protected_effect_audit/permission_tests.rs"]
mod permission_tests;
#[cfg(test)]
#[path = "protected_effect_audit/tests.rs"]
mod tests;

pub struct HubProtectedEffectAudit {
    connection: Arc<Connection<Listening>>,
}

impl HubProtectedEffectAudit {
    pub fn new(connection: Arc<Connection<Listening>>) -> Self {
        Self { connection }
    }
}

pub fn client(connection: Arc<Connection<Listening>>) -> Arc<dyn ProtectedEffectAudit> {
    Arc::new(HubProtectedEffectAudit::new(connection))
}

fn encode_request(request: &impl serde::Serialize) -> loopal_error::Result<serde_json::Value> {
    serde_json::to_value(request)
        .map_err(|error| LoopalError::Other(format!("protected audit encode failed: {error}")))
}

impl HubProtectedEffectAudit {
    async fn send(
        &self,
        method: &str,
        request: &impl serde::Serialize,
    ) -> loopal_error::Result<serde_json::Value> {
        let params = encode_request(request)?;
        tokio::time::timeout(AUDIT_TIMEOUT, self.connection.send_request(method, params))
            .await
            .map_err(|_| LoopalError::Other("protected audit timed out".into()))?
            .map_err(|error| LoopalError::Other(format!("protected audit RPC failed: {error}")))
    }
}

#[async_trait]
impl ProtectedEffectAudit for HubProtectedEffectAudit {
    async fn record(&self, request: &ProtectedEffectAuditRequest) -> loopal_error::Result<()> {
        request
            .validate()
            .map_err(|error| LoopalError::Other(error.to_string()))?;
        let response = self
            .send(methods::HUB_AUDIT_PROTECTED_EFFECT.name, request)
            .await?;
        let response: ProtectedEffectAuditResponse =
            serde_json::from_value(response).map_err(|error| {
                LoopalError::Other(format!("protected audit decode failed: {error}"))
            })?;
        require_recorded(response.recorded)
    }

    async fn record_permission_decision(
        &self,
        request: &PermissionDecisionAuditRequest,
    ) -> loopal_error::Result<()> {
        request
            .validate()
            .map_err(|error| LoopalError::Other(error.to_string()))?;
        let response = self
            .send(methods::HUB_AUDIT_PERMISSION_DECISION.name, request)
            .await?;
        let response: PermissionDecisionAuditResponse =
            serde_json::from_value(response).map_err(|error| {
                LoopalError::Other(format!("protected audit decode failed: {error}"))
            })?;
        require_recorded(response.recorded)
    }
}

fn require_recorded(recorded: bool) -> loopal_error::Result<()> {
    if recorded {
        Ok(())
    } else {
        Err(LoopalError::Other(
            "protected audit was not recorded".into(),
        ))
    }
}
