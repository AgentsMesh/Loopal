use serde_json::{Value, json};

pub(super) fn start() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["request_id", "spec"],
        "properties": {
            "request_id": id("Idempotency key"),
            "spec": loopal_protocol::workflow_spec_schema()
        }
    })
}

pub(super) fn get() -> Value {
    command_schema(
        json!({
            "request_id": id("Request identifier"),
            "run_id": id("Workflow run identifier")
        }),
        &["request_id", "run_id"],
    )
}

pub(super) fn wait() -> Value {
    command_schema(
        json!({
            "request_id": id("Request identifier"),
            "run_id": id("Workflow run identifier"),
            "after_revision": {"type": "integer", "minimum": 0},
            "timeout_ms": {"type": "integer", "minimum": 0, "maximum": 300000}
        }),
        &["request_id", "run_id", "after_revision", "timeout_ms"],
    )
}

pub(super) fn cancel() -> Value {
    command_schema(
        json!({
            "request_id": id("Idempotency key"),
            "run_id": id("Workflow run identifier"),
            "reason": {"type": ["string", "null"]}
        }),
        &["request_id", "run_id"],
    )
}

fn command_schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

fn id(description: &str) -> Value {
    json!({
        "type": "string", "minLength": 1, "maxLength": 128,
        "pattern": "^[A-Za-z0-9][A-Za-z0-9_-]*$", "description": description
    })
}
