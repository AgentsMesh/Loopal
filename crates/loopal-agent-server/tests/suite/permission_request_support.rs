use loopal_protocol::{PermissionIntent, PermissionIntentRequest, PermissionReceipt};
use serde_json::{Value, json};

pub(crate) fn permission_request(id: &str, name: &str, input: Value) -> PermissionIntentRequest {
    PermissionIntentRequest::create(
        id,
        name,
        input.clone(),
        input,
        json!({"type": "object"}),
        None,
    )
    .unwrap()
}

pub(crate) fn permission_receipt(request: &PermissionIntentRequest) -> PermissionReceipt {
    let intent =
        PermissionIntent::bind(request.intent_seed.clone(), 1, 1, "test-interaction-token")
            .unwrap();
    PermissionReceipt::issue_for_intent(&intent, "test-audit-issuance").unwrap()
}
