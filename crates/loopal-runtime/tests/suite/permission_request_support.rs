use loopal_protocol::PermissionIntentRequest;
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
