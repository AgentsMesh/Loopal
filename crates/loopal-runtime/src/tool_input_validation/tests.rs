use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use super::{validate_schema, validate_tool_input, validate_wire_refs};

struct SchemaTool(Value);

#[async_trait]
impl Tool for SchemaTool {
    fn name(&self) -> &str {
        "SchemaTool"
    }

    fn description(&self) -> &str {
        "test schema"
    }

    fn parameters_schema(&self) -> Value {
        self.0.clone()
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, _: Value, _: &ToolContext) -> Result<ToolResult, LoopalError> {
        unreachable!()
    }
}

fn result(schema: Value, value: Value) -> Result<(), String> {
    validate_schema(&schema, &value, "$", &schema)
}

#[test]
fn boolean_and_reference_schemas_are_enforced() {
    assert!(result(Value::Null, json!("anything")).is_ok());
    assert!(result(Value::Bool(true), json!("anything")).is_ok());
    assert!(result(Value::Bool(false), json!("anything")).is_err());

    let schema = json!({
        "$defs": {"name": {"type": "string"}},
        "$ref": "#/$defs/name"
    });
    assert!(result(schema.clone(), json!("ok")).is_ok());
    assert!(result(schema, json!(1)).is_err());
    assert!(
        result(json!({"$ref": "other.json#/name"}), json!("x"))
            .unwrap_err()
            .contains("unsupported external")
    );
    assert!(
        result(json!({"$ref": "#/$defs/missing"}), json!("x"))
            .unwrap_err()
            .contains("unresolved schema")
    );
}

#[test]
fn constants_enums_types_and_compositions_form_a_matrix() {
    assert!(result(json!({"const": "fixed"}), json!("fixed")).is_ok());
    assert!(result(json!({"const": "fixed"}), json!("other")).is_err());
    assert!(result(json!({"enum": ["a", "b"]}), json!("c")).is_err());
    assert!(result(json!({"type": ["string", "null"]}), Value::Null).is_ok());
    assert!(result(json!({"type": "integer"}), json!(1.5)).is_err());
    assert!(result(json!({"type": "future_type"}), json!(1.5)).is_ok());
    assert!(
        result(
            json!({"allOf": [{"type": "string"}, {"const": "yes"}]}),
            json!("no")
        )
        .is_err()
    );
    assert!(
        result(
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            json!(7)
        )
        .is_err()
    );
    assert!(
        result(
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            json!("allowed")
        )
        .is_ok()
    );
    assert!(
        result(
            json!({"oneOf": [{"type": "number"}, {"type": "integer"}]}),
            json!(7)
        )
        .is_err()
    );
    assert!(
        result(
            json!({"oneOf": [{"type": "string"}, {"type": "null"}]}),
            json!(true)
        )
        .is_err()
    );
    assert!(
        result(
            json!({"oneOf": [{"type": "string"}, {"type": "null"}]}),
            json!("exactly-one")
        )
        .is_ok()
    );
}

#[test]
fn objects_arrays_and_empty_optionals_are_validated_recursively() {
    let schema = json!({
        "type": "object",
        "properties": {
            "required": {"type": "string"},
            "optional": {"type": "integer"},
            "items": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["required"],
        "additionalProperties": false
    });
    let tool = SchemaTool(schema);
    assert!(validate_tool_input(&tool, &json!({"required": "", "optional": ""})).is_ok());
    assert!(validate_tool_input(&tool, &json!({"optional": 1})).is_err());
    assert!(validate_tool_input(&tool, &json!({"required": "x", "extra": 1})).is_err());
    assert!(validate_tool_input(&tool, &json!({"required": "x", "items": ["x", 1]})).is_err());

    let extras = json!({
        "type": "object",
        "additionalProperties": {"type": "integer"}
    });
    assert!(result(extras, json!({"ok": 1, "bad": "x"})).is_err());
    assert!(result(json!({}), json!("schema-free")).is_ok());
    assert!(result(json!({"type": "object"}), json!({"free": 1})).is_ok());
    assert!(result(json!({"type": 7}), json!("unknown-type-shape")).is_ok());
    assert!(
        result(
            json!({"type": "object", "additionalProperties": {"type": "integer"}}),
            json!({"ok": 1})
        )
        .is_ok()
    );
}

#[test]
fn wire_refs_are_allowed_only_below_eligible_fields() {
    let valid = json!({
        "env": {"TOKEN": ["prefix-<secret_ref:api_key>"]},
        "command": "safe"
    });
    assert!(validate_wire_refs(&valid, &["env"]).is_ok());
    assert!(
        validate_wire_refs(&json!({"command": "<secret_ref:key>"}), &["env"])
            .unwrap_err()
            .contains("non-secret-eligible")
    );
    for malformed in [
        "<secret_ref:Bad>",
        "<secret_ref:bad-name>",
        "<secret_ref:bad-name><secret_ref:good>",
    ] {
        assert!(
            validate_wire_refs(&json!({"env": malformed}), &["env"])
                .unwrap_err()
                .contains("malformed")
        );
    }
}
