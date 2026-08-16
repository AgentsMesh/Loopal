use loopal_protocol::{PermissionIntentRequest, PermissionRequestError};
use serde_json::json;

#[test]
fn request_recomputes_action_display_and_schema_digests() {
    let request = PermissionIntentRequest::create(
        "tool-1",
        "Bash",
        json!({"command": "ls"}),
        json!({
            "command": "ls",
            "sandbox_approval_reason": "outside sandbox",
        }),
        json!({"type": "object"}),
        None,
    )
    .unwrap();
    request.validate().unwrap();
    let wire = serde_json::to_value(&request).unwrap();
    assert!(wire.get("tool_input").is_some());
    assert!(wire.get("permission_intent").is_some());
    assert!(wire.get("display_input").is_none());
    assert!(wire.get("intent_seed").is_none());

    for field in ["action_input", "tool_input", "tool_schema"] {
        let mut tampered = wire.clone();
        tampered[field] = json!({"tampered": true});
        let decoded: PermissionIntentRequest = serde_json::from_value(tampered).unwrap();
        assert!(decoded.validate().is_err(), "{field}");
    }
}

#[test]
fn display_input_may_only_add_reserved_sandbox_reason() {
    let schema = json!({"type": "object"});
    for display in [
        json!("ls"),
        json!({"command": "rm -rf /"}),
        json!({"command": "ls", "extra": "misleading"}),
        json!({"command": "ls", "sandbox_approval_reason": ""}),
    ] {
        assert!(
            PermissionIntentRequest::create(
                "tool-1",
                "Bash",
                json!({"command": "ls"}),
                display,
                schema.clone(),
                None,
            )
            .is_err()
        );
    }
}

#[test]
fn validation_reports_each_bound_field_mismatch() {
    let request = PermissionIntentRequest::create(
        "tool-1",
        "Bash",
        json!({"command": "ls"}),
        json!({"command": "ls"}),
        json!({"type": "object"}),
        None,
    )
    .unwrap();

    for tool_call_id in ["", "bad\nid"] {
        let mut invalid = request.clone();
        invalid.tool_call_id = tool_call_id.into();
        assert_eq!(invalid.validate(), Err(PermissionRequestError::ToolCallId));
    }

    let mut invalid = request.clone();
    invalid.tool_name = "Read".into();
    assert_eq!(invalid.validate(), Err(PermissionRequestError::ToolName));

    let mut invalid = request.clone();
    invalid.action_input = json!({"command": "pwd"});
    invalid.display_input = invalid.action_input.clone();
    assert_eq!(
        invalid.validate(),
        Err(PermissionRequestError::ActionDigest)
    );

    let mut invalid = request.clone();
    invalid.display_input = json!({
        "command": "ls",
        "sandbox_approval_reason": "outside sandbox",
    });
    assert_eq!(
        invalid.validate(),
        Err(PermissionRequestError::DisplayDigest)
    );
}

#[test]
fn validation_errors_have_stable_messages() {
    let cases = [
        (
            PermissionRequestError::ToolCallId,
            "invalid permission tool call id",
        ),
        (
            PermissionRequestError::ToolName,
            "permission intent tool name mismatch",
        ),
        (
            PermissionRequestError::DisplayInput,
            "permission display input is not derived from the action",
        ),
        (
            PermissionRequestError::ActionDigest,
            "permission action digest mismatch",
        ),
        (
            PermissionRequestError::DisplayDigest,
            "permission display digest mismatch",
        ),
        (
            PermissionRequestError::SchemaDigest,
            "permission schema digest mismatch",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn malformed_wire_is_rejected_or_fails_validation() {
    let request = PermissionIntentRequest::create(
        "tool-1",
        "Bash",
        json!({}),
        json!({}),
        json!({"type": "object"}),
        None,
    )
    .unwrap();
    let wire = serde_json::to_value(request).unwrap();
    for field in ["tool_call_id", "tool_name", "permission_intent"] {
        let mut missing = wire.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<PermissionIntentRequest>(missing).is_err());
    }
    let mut unknown = wire;
    unknown["unknown"] = json!(true);
    assert!(serde_json::from_value::<PermissionIntentRequest>(unknown).is_err());
}
