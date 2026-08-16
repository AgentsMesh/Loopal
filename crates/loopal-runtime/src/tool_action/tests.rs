use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::Settings;
use loopal_error::{LoopalError, ToolError};
use loopal_kernel::Kernel;
use loopal_protocol::PermissionActionDigest;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use super::{PreparedToolAction, action_digest, invalid_permission_intent};

struct InertTool;

#[async_trait]
impl Tool for InertTool {
    fn name(&self) -> &str {
        "Inert"
    }

    fn description(&self) -> &str {
        "test tool"
    }

    fn parameters_schema(&self) -> Value {
        json!({"type": "object"})
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        panic!("integrity test must not execute the tool")
    }
}

#[test]
fn canonical_digest_ignores_object_key_order() {
    let left = json!({"a": 1, "nested": {"x": true, "y": false}});
    let right: serde_json::Value =
        serde_json::from_str(r#"{"nested":{"y":false,"x":true},"a":1}"#).unwrap();
    assert_eq!(
        action_digest("id", "Tool", &left),
        action_digest("id", "Tool", &right)
    );
}

#[test]
fn canonical_digest_preserves_array_order() {
    assert_ne!(
        action_digest("id", "Tool", &json!({"items": [1, 2]})),
        action_digest("id", "Tool", &json!({"items": [2, 1]})),
    );
}

fn action(input: Value) -> PreparedToolAction {
    let kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_tool(Box::new(InertTool));
    PreparedToolAction::new(
        "id".into(),
        "Inert".into(),
        input,
        kernel.get_tool("Inert").unwrap(),
        false,
    )
}

#[test]
fn invalid_permission_intent_is_a_tool_input_error() {
    let error = invalid_permission_intent("bad seed");
    assert!(
        matches!(error, LoopalError::Tool(ToolError::InvalidInput(message)) if message.contains("bad seed"))
    );
}

#[test]
fn digest_tampering_fails_closed() {
    let kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_tool(Box::new(InertTool));
    let tool = kernel.get_tool("Inert").unwrap();
    let mut action = PreparedToolAction::new(
        "id".into(),
        "Inert".into(),
        json!({"value": "approved"}),
        Arc::clone(&tool),
        false,
    );
    let mut bytes = *action.digest.as_bytes();
    bytes[0] ^= 0xff;
    action.digest = PermissionActionDigest::from_bytes(bytes);
    assert!(action.verify(&kernel).is_err());
}

#[test]
fn tool_identity_tampering_fails_before_digest_validation() {
    let kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_tool(Box::new(InertTool));
    let mut action = action(json!({"value": "approved"}));
    action.tool_name = "Other".into();
    assert!(action.verify(&kernel).is_err());
}

#[test]
fn permission_annotation_changes_display_only() {
    let mut action = action(json!({"value": "approved"}));
    action
        .annotate_permission("outside workspace".into())
        .unwrap();

    let request = action.permission_request(None).unwrap();

    assert_eq!(request.action_input, json!({"value": "approved"}));
    assert_eq!(
        request.display_input,
        json!({
            "value": "approved",
            "sandbox_approval_reason": "outside workspace"
        })
    );
    request.validate().unwrap();
}

#[test]
fn annotation_rejects_non_object_and_reserved_key() {
    let mut scalar = action(json!("value"));
    assert!(scalar.annotate_permission("reason".into()).is_err());

    let mut collision = action(json!({"sandbox_approval_reason": "model"}));
    assert!(collision.annotate_permission("runtime".into()).is_err());
}
