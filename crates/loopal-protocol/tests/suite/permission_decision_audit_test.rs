use loopal_protocol::{
    PermissionActionDigest, PermissionAuditDecision, PermissionAuditSource,
    PermissionDecisionAuditRequest, PermissionDecisionAuditResponse, PermissionIntentDigest,
    PermissionSchemaDigest,
};

fn request() -> PermissionDecisionAuditRequest {
    PermissionDecisionAuditRequest::new(
        "call-1",
        "Bash",
        PermissionActionDigest::from_bytes([0x11; 32]),
        PermissionSchemaDigest::from_bytes([0x22; 32]),
        Some(PermissionIntentDigest::from_bytes([0x33; 32])),
        PermissionAuditDecision::Allow,
        PermissionAuditSource::Ui,
    )
    .unwrap()
}

#[test]
fn request_roundtrips_without_sensitive_payloads() {
    let value = serde_json::to_value(request()).unwrap();
    assert_eq!(value["tool_call_id"], "call-1");
    assert_eq!(value["decision"], "allow");
    assert_eq!(value["source"], "ui");
    for field in ["action_input", "tool_input", "tool_schema", "reason"] {
        assert!(value.get(field).is_none());
    }
    assert_eq!(
        serde_json::from_value::<PermissionDecisionAuditRequest>(value).unwrap(),
        request()
    );
}

#[test]
fn request_rejects_bad_text_and_unknown_fields() {
    let action = PermissionActionDigest::from_bytes([1; 32]);
    let schema = PermissionSchemaDigest::from_bytes([2; 32]);
    for (id, tool) in [("", "Bash"), ("id", ""), ("bad\nid", "Bash")] {
        assert!(
            PermissionDecisionAuditRequest::new(
                id,
                tool,
                action,
                schema,
                None,
                PermissionAuditDecision::Deny,
                PermissionAuditSource::Policy,
            )
            .is_err()
        );
    }
    assert!(
        PermissionDecisionAuditRequest::new(
            "x".repeat(257),
            "Bash",
            action,
            schema,
            None,
            PermissionAuditDecision::Deny,
            PermissionAuditSource::Policy,
        )
        .is_err()
    );
    let mut value = serde_json::to_value(request()).unwrap();
    value["action_input"] = serde_json::json!({"secret": "forbidden"});
    assert!(serde_json::from_value::<PermissionDecisionAuditRequest>(value).is_err());
}

#[test]
fn accessors_and_enum_strings_are_stable() {
    let request = request();
    assert_eq!(request.tool_call_id(), "call-1");
    assert_eq!(request.tool_name(), "Bash");
    assert_eq!(request.decision().as_str(), "allow");
    assert_eq!(request.source().as_str(), "ui");
    assert!(request.intent_digest().is_some());
    assert_eq!(PermissionAuditDecision::Deny.as_str(), "deny");
    assert_eq!(PermissionAuditSource::Frontend.as_str(), "frontend");
    assert_eq!(PermissionAuditSource::Policy.as_str(), "policy");
    assert_eq!(
        PermissionAuditSource::RememberedGrant.as_str(),
        "remembered_grant"
    );
    request.validate().unwrap();
}

#[test]
fn response_and_error_contracts_are_typed() {
    assert_eq!(
        serde_json::to_value(PermissionDecisionAuditResponse { recorded: true }).unwrap(),
        serde_json::json!({"recorded": true})
    );
    let error = PermissionDecisionAuditRequest::new(
        "",
        "Bash",
        PermissionActionDigest::from_bytes([1; 32]),
        PermissionSchemaDigest::from_bytes([2; 32]),
        None,
        PermissionAuditDecision::Deny,
        PermissionAuditSource::Policy,
    )
    .unwrap_err();
    let as_error: &dyn std::error::Error = &error;
    assert!(as_error.to_string().contains("permission audit text"));
}
