use serde_json::{Value, json};

use super::{
    MAX_DEPENDENCIES_PER_NODE, MAX_WORKER_PROFILE_BYTES, MAX_WORKFLOW_ATTEMPT_TIMEOUT_MS,
    MAX_WORKFLOW_ATTEMPTS, MAX_WORKFLOW_GOAL_BYTES, MAX_WORKFLOW_ID_BYTES, MAX_WORKFLOW_NODES,
    MAX_WORKFLOW_OUTPUT_BYTES, MAX_WORKFLOW_PARALLELISM, MAX_WORKFLOW_RUN_DEADLINE_MS,
    MAX_WORKFLOW_TASK_BYTES, WORKFLOW_PLAN_V1, WORKFLOW_SPEC_V1,
};

/// Canonical JSON Schema for the provider-neutral planner response.
pub fn workflow_plan_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["version", "execution"],
        "properties": {
            "version": {"const": WORKFLOW_PLAN_V1},
            "execution": {
                "oneOf": [
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["kind"],
                        "properties": {
                            "kind": {"const": "direct"},
                            "reason": {"type": "string", "maxLength": 512}
                        }
                    },
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["kind", "spec"],
                        "properties": {
                            "kind": {"const": "workflow"},
                            "spec": workflow_spec_schema()
                        }
                    }
                ]
            }
        }
    })
}

/// Canonical wire schema for a V1 workflow specification.
///
/// Cross-field and graph invariants remain the responsibility of
/// `validate_workflow_spec`; JSON Schema cannot portably express all of them.
pub fn workflow_spec_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "version", "run_goal", "nodes", "limits", "output_node", "output_contract"
        ],
        "properties": {
            "version": {"const": WORKFLOW_SPEC_V1},
            "run_goal": {
                "type": "string", "minLength": 1, "maxLength": MAX_WORKFLOW_GOAL_BYTES
            },
            "nodes": {
                "type": "array", "minItems": 1, "maxItems": MAX_WORKFLOW_NODES,
                "items": workflow_node_schema()
            },
            "limits": workflow_limits_schema(),
            "output_node": workflow_id_schema(),
            "output_contract": workflow_output_contract_schema()
        }
    })
}

fn workflow_node_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "dependencies", "task", "worker_profile"],
        "properties": {
            "id": workflow_id_schema(),
            "dependencies": {
                "type": "array", "maxItems": MAX_DEPENDENCIES_PER_NODE,
                "uniqueItems": true, "items": workflow_id_schema()
            },
            "task": {
                "type": "string", "minLength": 1, "maxLength": MAX_WORKFLOW_TASK_BYTES
            },
            "worker_profile": {
                "type": "string", "minLength": 1, "maxLength": MAX_WORKER_PROFILE_BYTES,
                "pattern": "^[A-Za-z0-9][A-Za-z0-9_-]*$"
            }
        }
    })
}

fn workflow_limits_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "max_nodes", "max_parallel", "max_attempts", "run_deadline_ms",
            "attempt_timeout_ms", "max_output_bytes"
        ],
        "properties": {
            "max_nodes": integer_limit(MAX_WORKFLOW_NODES),
            "max_parallel": integer_limit(MAX_WORKFLOW_PARALLELISM),
            "max_attempts": integer_limit(MAX_WORKFLOW_ATTEMPTS),
            "run_deadline_ms": integer_limit(MAX_WORKFLOW_RUN_DEADLINE_MS),
            "attempt_timeout_ms": integer_limit(MAX_WORKFLOW_ATTEMPT_TIMEOUT_MS),
            "max_output_bytes": integer_limit(MAX_WORKFLOW_OUTPUT_BYTES)
        }
    })
}

fn workflow_output_contract_schema() -> Value {
    let max_bytes = integer_limit(MAX_WORKFLOW_OUTPUT_BYTES);
    json!({
        "oneOf": [
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "max_bytes"],
                "properties": {"type": {"const": "text"}, "max_bytes": max_bytes}
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["type", "max_bytes", "schema"],
                "properties": {
                    "type": {"const": "json"},
                    "max_bytes": max_bytes,
                    "schema": {"type": "object"}
                }
            }
        ]
    })
}

fn workflow_id_schema() -> Value {
    json!({
        "type": "string", "minLength": 1, "maxLength": MAX_WORKFLOW_ID_BYTES,
        "pattern": "^[A-Za-z0-9][A-Za-z0-9_-]*$"
    })
}

fn integer_limit(maximum: impl serde::Serialize) -> Value {
    json!({"type": "integer", "minimum": 1, "maximum": maximum})
}
