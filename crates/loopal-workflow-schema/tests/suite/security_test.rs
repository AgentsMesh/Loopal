use loopal_workflow_schema::{
    WorkflowSchemaError, validate_workflow_json, validate_workflow_schema,
};
use serde_json::json;

#[test]
fn rejects_wrong_or_malformed_dialect() {
    for schema in [
        json!({"$schema": "http://json-schema.org/draft-07/schema#", "type": "string"}),
        json!({"$schema": 42, "type": "string"}),
        json!({"allOf": [{"$schema": "https://example.test/custom", "type": "string"}]}),
    ] {
        assert!(validate_workflow_schema(&schema).is_err());
    }
}

#[test]
fn rejects_external_references_at_every_depth() {
    for schema in [
        json!({"$ref": "https://example.test/schema"}),
        json!({"properties": {"x": {"$ref": "file:///tmp/schema.json"}}}),
        json!({"$dynamicRef": "https://example.test/schema#node"}),
        json!({"$recursiveRef": "other.json#"}),
    ] {
        assert_eq!(
            validate_workflow_schema(&schema),
            Err(WorkflowSchemaError::ExternalReference)
        );
    }
}

#[test]
fn malformed_or_missing_local_reference_fails_closed() {
    for schema in [
        json!({"$ref": 7}),
        json!({"$ref": "#/$defs/missing"}),
        json!({
            "definitions": {"hidden": {"$ref": "https://example.test/schema"}},
            "$ref": "#/definitions/hidden"
        }),
    ] {
        assert_eq!(
            validate_workflow_schema(&schema),
            Err(WorkflowSchemaError::InvalidSchema)
        );
    }
}

#[test]
fn rejects_invalid_schema_and_non_object_root() {
    assert_eq!(
        validate_workflow_schema(&json!({"type": "not-a-json-type"})),
        Err(WorkflowSchemaError::InvalidSchema)
    );
    assert_eq!(
        validate_workflow_schema(&json!([{"type": "string"}])),
        Err(WorkflowSchemaError::Bounds)
    );
}

#[test]
fn linear_regex_policy_rejects_backtracking_extensions() {
    assert_eq!(
        validate_workflow_schema(&json!({"type": "string", "pattern": "(?=a)a"})),
        Err(WorkflowSchemaError::InvalidSchema)
    );
}

#[test]
fn errors_do_not_echo_schema_or_instance_values() {
    let schema_secret = "schema-secret-sentinel";
    let output_secret = "output-secret-sentinel";
    let invalid_schema = json!({"type": "invalid", "description": schema_secret});
    let schema_error = validate_workflow_schema(&invalid_schema)
        .unwrap_err()
        .to_string();
    let output_error = validate_workflow_json(&json!({"type": "integer"}), &json!(output_secret))
        .unwrap_err()
        .to_string();
    assert!(!schema_error.contains(schema_secret));
    assert!(!output_error.contains(output_secret));
}

#[test]
fn schema_protocol_bounds_are_enforced() {
    let schema =
        json!({"description": "x".repeat(loopal_protocol::MAX_JSON_SCHEMA_STRING_BYTES + 1)});
    assert_eq!(
        validate_workflow_schema(&schema),
        Err(WorkflowSchemaError::Bounds)
    );
}
