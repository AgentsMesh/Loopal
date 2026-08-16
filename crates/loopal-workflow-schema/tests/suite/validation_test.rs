use loopal_protocol::WorkflowJsonValidator;
use loopal_workflow_schema::{
    WorkflowSchemaError, WorkflowSchemaValidator, validate_workflow_json, validate_workflow_schema,
};
use serde_json::json;

fn object_schema() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
            "answer": {"type": "integer", "minimum": 1},
            "state": {"enum": ["done"]}
        },
        "required": ["answer", "state"],
        "additionalProperties": false
    })
}

#[test]
fn validates_draft_2020_12_schema_and_instance() {
    let schema = object_schema();
    validate_workflow_schema(&schema).unwrap();
    validate_workflow_json(&schema, &json!({"answer": 42, "state": "done"})).unwrap();
}

#[test]
fn rejects_type_required_and_additional_property_violations() {
    let schema = object_schema();
    for value in [
        json!({"answer": "42", "state": "done"}),
        json!({"answer": 42}),
        json!({"answer": 42, "state": "done", "extra": true}),
    ] {
        assert_eq!(
            validate_workflow_json(&schema, &value),
            Err(WorkflowSchemaError::InstanceMismatch)
        );
    }
}

#[test]
fn protocol_validator_trait_uses_concrete_semantics() {
    let validator = WorkflowSchemaValidator;
    let schema = json!({"type": "array", "items": {"type": "boolean"}});
    validator.validate(&schema, &json!([true, false])).unwrap();
    assert_eq!(
        validator.validate(&schema, &json!([true, 1])),
        Err(WorkflowSchemaError::InstanceMismatch)
    );
}

#[test]
fn local_pointer_and_anchor_references_work() {
    for schema in [
        json!({
            "$defs": {"positive": {"type": "integer", "minimum": 1}},
            "$ref": "#/$defs/positive"
        }),
        json!({
            "$defs": {"positive": {"$anchor": "positive", "type": "integer", "minimum": 1}},
            "$ref": "#positive"
        }),
    ] {
        validate_workflow_json(&schema, &json!(1)).unwrap();
        assert_eq!(
            validate_workflow_json(&schema, &json!(0)),
            Err(WorkflowSchemaError::InstanceMismatch)
        );
    }
}

#[test]
fn enforces_assertion_formats() {
    let schema = json!({"type": "string", "format": "uuid"});
    validate_workflow_json(&schema, &json!("7f84ca90-2f24-4ea0-8a16-12b72842c47f")).unwrap();
    assert_eq!(
        validate_workflow_json(&schema, &json!("not-a-uuid")),
        Err(WorkflowSchemaError::InstanceMismatch)
    );
}
