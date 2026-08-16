use loopal_ipc::protocol::methods;
use loopal_protocol::{
    PermissionDecisionAuditRequest, PermissionDecisionAuditResponse, PermissionIntentRequest,
    PermissionReceipt, ProtectedEffectAuditRequest, ProtectedEffectAuditResponse,
};
use serde_json::{Value, json};

pub(super) fn audit_reply(method: &str, params: &Value) -> Option<Result<Value, String>> {
    if method == methods::HUB_AUDIT_PROTECTED_EFFECT.name {
        let valid = serde_json::from_value::<ProtectedEffectAuditRequest>(params.clone())
            .is_ok_and(|request| request.validate().is_ok());
        return Some(if valid {
            Ok(json!(ProtectedEffectAuditResponse { recorded: true }))
        } else {
            Err("invalid protected effect audit request".into())
        });
    }
    if method == methods::HUB_AUDIT_PERMISSION_DECISION.name {
        let valid = serde_json::from_value::<PermissionDecisionAuditRequest>(params.clone())
            .is_ok_and(|request| request.validate().is_ok());
        return Some(if valid {
            Ok(json!(PermissionDecisionAuditResponse { recorded: true }))
        } else {
            Err("invalid permission decision audit request".into())
        });
    }
    None
}

pub(super) fn permission_allow_reply(params: &Value) -> Result<Value, String> {
    let request: PermissionIntentRequest = serde_json::from_value(params.clone())
        .map_err(|error| format!("invalid permission intent request: {error}"))?;
    request
        .validate()
        .map_err(|error| format!("invalid permission intent request: {error}"))?;
    let receipt = PermissionReceipt::issue_local(&request.intent_seed, "cli-e2e-harness")
        .map_err(|error| format!("permission receipt issuance failed: {error}"))?;
    Ok(json!({"allow": true, "permission_receipt": receipt}))
}
