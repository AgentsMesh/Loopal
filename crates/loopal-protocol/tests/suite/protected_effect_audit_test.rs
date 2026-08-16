use loopal_protocol::{
    PermissionActionDigest, PermissionSchemaDigest, ProtectedEffectAuditRequest,
    ProtectedEffectAuditResponse,
};

fn request() -> ProtectedEffectAuditRequest {
    ProtectedEffectAuditRequest::new(
        "tool-call-1",
        "Bash",
        PermissionActionDigest::from_bytes([0x11; 32]),
        PermissionSchemaDigest::from_bytes([0x22; 32]),
    )
    .unwrap()
}

#[test]
fn request_roundtrips_without_action_input() {
    let value = serde_json::to_value(request()).unwrap();
    assert_eq!(value["tool_call_id"], "tool-call-1");
    assert_eq!(value["tool_name"], "Bash");
    assert!(value.get("action_input").is_none());
    assert!(value.get("session_id").is_none());
    assert!(value.get("agent_name").is_none());

    let restored: ProtectedEffectAuditRequest = serde_json::from_value(value).unwrap();
    assert_eq!(restored, request());
}

#[test]
fn request_rejects_empty_oversized_and_unknown_fields() {
    let action = PermissionActionDigest::from_bytes([1; 32]);
    let schema = PermissionSchemaDigest::from_bytes([2; 32]);
    assert!(ProtectedEffectAuditRequest::new("", "Bash", action, schema).is_err());
    assert!(ProtectedEffectAuditRequest::new("id", "", action, schema).is_err());
    assert!(ProtectedEffectAuditRequest::new("x".repeat(513), "Bash", action, schema).is_err());
    assert!(ProtectedEffectAuditRequest::new("id", "x".repeat(257), action, schema).is_err());

    let mut value = serde_json::to_value(request()).unwrap();
    value["action_input"] = serde_json::json!({"secret": "forbidden"});
    assert!(serde_json::from_value::<ProtectedEffectAuditRequest>(value).is_err());
}

#[test]
fn request_accessors_and_exact_limits_are_stable() {
    let action = PermissionActionDigest::from_bytes([0x11; 32]);
    let schema = PermissionSchemaDigest::from_bytes([0x22; 32]);
    let request =
        ProtectedEffectAuditRequest::new("x".repeat(512), "y".repeat(256), action, schema).unwrap();

    assert_eq!(request.tool_call_id().len(), 512);
    assert_eq!(request.tool_name().len(), 256);
    assert_eq!(request.action_digest(), action);
    assert_eq!(request.schema_digest(), schema);
    request.validate().unwrap();
}

#[test]
fn audit_error_has_stable_display_and_error_contract() {
    let error = ProtectedEffectAuditRequest::new(
        "",
        "Bash",
        PermissionActionDigest::from_bytes([1; 32]),
        PermissionSchemaDigest::from_bytes([2; 32]),
    )
    .unwrap_err();
    let as_error: &dyn std::error::Error = &error;

    assert_eq!(
        as_error.to_string(),
        "protected effect audit text must be non-empty and within its byte limit"
    );
}

#[test]
fn response_is_typed_ack() {
    let response = ProtectedEffectAuditResponse { recorded: true };
    assert_eq!(
        serde_json::to_value(response).unwrap(),
        serde_json::json!({"recorded": true})
    );
}
