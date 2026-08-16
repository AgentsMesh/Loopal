use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tracing::{info, warn};

use loopal_ipc::protocol::methods;
use loopal_protocol::{
    DEFAULT_INTERACTION_RPC_TIMEOUT, PermissionIntentRequest, PermissionReceipt,
};
use loopal_runtime::frontend::permission_handler::{PermissionHandler, PermissionOutcome};

use super::{SessionRef, primary_connection, send_interaction_request};

pub(super) fn permission_outcome_from_response(
    request: &PermissionIntentRequest,
    value: &Value,
) -> PermissionOutcome {
    if value.get("allow").and_then(Value::as_bool) != Some(true) {
        return PermissionOutcome::deny("user denied");
    }
    let receipt = match value
        .get("permission_receipt")
        .cloned()
        .map(serde_json::from_value::<PermissionReceipt>)
    {
        Some(Ok(receipt)) => receipt,
        Some(Err(error)) => {
            warn!(tool = %request.tool_name, %error, "permission denied: malformed receipt");
            return PermissionOutcome::deny("invalid permission receipt");
        }
        None => {
            warn!(tool = %request.tool_name, "permission denied: missing receipt");
            return PermissionOutcome::deny("missing permission receipt");
        }
    };
    if let Err(error) = receipt.validate_for(&request.intent_seed) {
        warn!(tool = %request.tool_name, %error, "permission denied: receipt binding mismatch");
        return PermissionOutcome::deny("invalid permission receipt binding");
    }
    let mut outcome = PermissionOutcome::allow();
    outcome.receipt = Some(receipt);
    outcome
}

pub struct IpcPermissionHandler {
    session: SessionRef,
    request_timeout: Duration,
}

impl IpcPermissionHandler {
    pub fn new(session: SessionRef) -> Self {
        Self {
            session,
            request_timeout: DEFAULT_INTERACTION_RPC_TIMEOUT,
        }
    }

    #[cfg(test)]
    pub(super) fn with_timeout(session: SessionRef, request_timeout: Duration) -> Self {
        Self {
            session,
            request_timeout,
        }
    }
}

#[async_trait]
impl PermissionHandler for IpcPermissionHandler {
    async fn decide(&self, request: &PermissionIntentRequest) -> PermissionOutcome {
        let name = &request.tool_name;
        info!(tool = name, "requesting permission via IPC");
        if let Err(error) = request.validate() {
            warn!(tool = name, %error, "permission denied: invalid intent request");
            return PermissionOutcome::deny("invalid permission intent request");
        }
        let Some(connection) = primary_connection(&self.session).await else {
            warn!(tool = name, "permission denied: no primary connection");
            return PermissionOutcome::deny("no primary connection");
        };
        let params = match serde_json::to_value(request) {
            Ok(params) => params,
            Err(error) => {
                warn!(tool = name, %error, "permission denied: request encoding failed");
                return PermissionOutcome::deny("permission request encoding failed");
            }
        };
        match send_interaction_request(
            &connection,
            methods::AGENT_PERMISSION.name,
            params,
            self.request_timeout,
        )
        .await
        {
            Ok(value) => {
                let allow = value.get("allow").and_then(Value::as_bool).unwrap_or(false);
                info!(tool = name, allow, "permission response received");
                permission_outcome_from_response(request, &value)
            }
            Err(error) => {
                warn!(tool = name, %error, "permission IPC failed");
                PermissionOutcome::deny(format!("ipc failure: {error}"))
            }
        }
    }
}
