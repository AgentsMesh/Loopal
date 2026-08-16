use loopal_protocol::{PermissionIntent, PermissionIntentRequest, PermissionReceipt};
use loopal_tool_api::PermissionDecision;

use super::permission_outcome_from_response;

fn request(id: &str) -> PermissionIntentRequest {
    PermissionIntentRequest::create(
        id,
        "Write",
        serde_json::json!({"file_path": "/tmp/output"}),
        serde_json::json!({"file_path": "/tmp/output"}),
        serde_json::json!({"type": "object"}),
        None,
    )
    .unwrap()
}

fn receipt(request: &PermissionIntentRequest) -> PermissionReceipt {
    let intent =
        PermissionIntent::bind(request.intent_seed.clone(), 7, 3, "interaction-token").unwrap();
    PermissionReceipt::issue_for_intent(&intent, "audit-issuance").unwrap()
}

#[test]
fn allow_requires_receipt() {
    let request = request("tool-allow-missing");
    let outcome = permission_outcome_from_response(&request, &serde_json::json!({"allow": true}));

    assert_eq!(outcome.decision, PermissionDecision::Deny);
    assert!(outcome.receipt.is_none());
    assert!(outcome.reason.contains("missing permission receipt"));
}

#[test]
fn allow_rejects_malformed_receipt() {
    let request = request("tool-allow-malformed");
    let response = serde_json::json!({
        "allow": true,
        "permission_receipt": {"interaction_token": "partial"},
    });
    let outcome = permission_outcome_from_response(&request, &response);

    assert_eq!(outcome.decision, PermissionDecision::Deny);
    assert!(outcome.reason.contains("invalid permission receipt"));
}

#[test]
fn allow_rejects_receipt_for_another_intent() {
    let requested = request("tool-requested");
    let other = request("tool-other");
    let response = serde_json::json!({
        "allow": true,
        "permission_receipt": receipt(&other),
    });
    let outcome = permission_outcome_from_response(&requested, &response);

    assert_eq!(outcome.decision, PermissionDecision::Deny);
    assert!(
        outcome
            .reason
            .contains("invalid permission receipt binding")
    );
}

#[test]
fn allow_accepts_exactly_bound_receipt() {
    let request = request("tool-allow-bound");
    let receipt = receipt(&request);
    let outcome = permission_outcome_from_response(
        &request,
        &serde_json::json!({"allow": true, "permission_receipt": receipt}),
    );

    assert_eq!(outcome.decision, PermissionDecision::Allow);
    assert_eq!(outcome.receipt, Some(receipt));
}
