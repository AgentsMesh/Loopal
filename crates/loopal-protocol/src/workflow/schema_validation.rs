use super::WorkflowValidationError;
use super::spec::*;

pub fn validate_workflow_schema_bounds(
    schema: &serde_json::Value,
) -> Result<(), WorkflowValidationError> {
    if !schema.is_object() {
        return Err(WorkflowValidationError::SchemaRootNotObject);
    }
    if serde_json::to_vec(schema)
        .expect("serde_json::Value always serializes")
        .len()
        > MAX_JSON_SCHEMA_BYTES
    {
        return Err(WorkflowValidationError::SchemaTooLarge);
    }
    let mut stack = vec![(schema, 1usize)];
    let mut count = 0;
    while let Some((value, depth)) = stack.pop() {
        count += 1;
        if count > MAX_JSON_SCHEMA_NODES {
            return Err(WorkflowValidationError::SchemaTooComplex);
        }
        if depth > MAX_JSON_SCHEMA_DEPTH {
            return Err(WorkflowValidationError::SchemaTooDeep);
        }
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    if key.len() > MAX_JSON_SCHEMA_KEY_BYTES {
                        return Err(WorkflowValidationError::SchemaKeyTooLong);
                    }
                    stack.push((child, depth + 1));
                }
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    stack.push((child, depth + 1));
                }
            }
            serde_json::Value::String(value) if value.len() > MAX_JSON_SCHEMA_STRING_BYTES => {
                return Err(WorkflowValidationError::SchemaStringTooLong);
            }
            _ => {}
        }
    }
    Ok(())
}
