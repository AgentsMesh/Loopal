use serde_json::Value;

use crate::permission_digest::{
    PermissionActionDigest, PermissionDisplayDigest, PermissionSchemaDigest, framed_sha256,
};

pub fn calculate_permission_action_digest(
    tool_call_id: &str,
    tool_name: &str,
    input: &Value,
) -> PermissionActionDigest {
    let canonical = canonical_json(input);
    PermissionActionDigest::from_bytes(framed_sha256(
        b"loopal.permission-action.v2",
        &[tool_call_id.as_bytes(), tool_name.as_bytes(), &canonical],
    ))
}

pub fn calculate_permission_display_digest(input: &Value) -> PermissionDisplayDigest {
    let canonical = canonical_json(input);
    PermissionDisplayDigest::from_bytes(framed_sha256(
        b"loopal.permission-display.v2",
        &[&canonical],
    ))
}

pub fn calculate_permission_schema_digest(schema: &Value) -> PermissionSchemaDigest {
    let canonical = canonical_json(schema);
    PermissionSchemaDigest::from_bytes(framed_sha256(b"loopal.permission-schema.v2", &[&canonical]))
}

fn canonical_json(value: &Value) -> Vec<u8> {
    let mut output = Vec::new();
    write_canonical(value, &mut output);
    output
}

fn write_canonical(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(value) => output.extend_from_slice(value.to_string().as_bytes()),
        Value::String(value) => {
            serde_json::to_writer(output, value).expect("serialize JSON string")
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical(value, output);
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, key).expect("serialize JSON key");
                output.push(b':');
                write_canonical(&values[key], output);
            }
            output.push(b'}');
        }
    }
}
