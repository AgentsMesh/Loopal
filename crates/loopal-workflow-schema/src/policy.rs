use serde_json::{Map, Value};

use crate::WorkflowSchemaError;

const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
const DIALECT_WITH_FRAGMENT: &str = "https://json-schema.org/draft/2020-12/schema#";
const SINGLE_SUBSCHEMAS: &[&str] = &[
    "additionalProperties",
    "contains",
    "contentSchema",
    "else",
    "if",
    "items",
    "not",
    "propertyNames",
    "then",
    "unevaluatedItems",
    "unevaluatedProperties",
];
const ARRAY_SUBSCHEMAS: &[&str] = &["allOf", "anyOf", "oneOf", "prefixItems"];
const MAP_SUBSCHEMAS: &[&str] = &[
    "$defs",
    "dependentSchemas",
    "patternProperties",
    "properties",
];

pub(crate) fn validate(schema: &Value) -> Result<(), WorkflowSchemaError> {
    let mut stack = vec![schema];
    while let Some(value) = stack.pop() {
        let Value::Object(object) = value else {
            continue;
        };
        validate_dialect(object.get("$schema"))?;
        for keyword in ["$ref", "$dynamicRef", "$recursiveRef"] {
            validate_reference(object.get(keyword))?;
        }
        push_subschemas(object, &mut stack);
    }
    Ok(())
}

fn validate_dialect(value: Option<&Value>) -> Result<(), WorkflowSchemaError> {
    match value {
        None => Ok(()),
        Some(Value::String(value)) if matches!(value.as_str(), DIALECT | DIALECT_WITH_FRAGMENT) => {
            Ok(())
        }
        Some(Value::String(_)) => Err(WorkflowSchemaError::UnsupportedDialect),
        Some(_) => Err(WorkflowSchemaError::InvalidSchema),
    }
}

fn validate_reference(value: Option<&Value>) -> Result<(), WorkflowSchemaError> {
    match value {
        None => Ok(()),
        Some(Value::String(value)) if value.is_empty() || value.starts_with('#') => Ok(()),
        Some(Value::String(_)) => Err(WorkflowSchemaError::ExternalReference),
        Some(_) => Err(WorkflowSchemaError::InvalidSchema),
    }
}

fn push_subschemas<'a>(object: &'a Map<String, Value>, stack: &mut Vec<&'a Value>) {
    for keyword in SINGLE_SUBSCHEMAS {
        if let Some(value) = object.get(*keyword) {
            stack.push(value);
        }
    }
    for keyword in ARRAY_SUBSCHEMAS {
        if let Some(Value::Array(values)) = object.get(*keyword) {
            stack.extend(values);
        }
    }
    for keyword in MAP_SUBSCHEMAS {
        if let Some(Value::Object(values)) = object.get(*keyword) {
            stack.extend(values.values());
        }
    }
}
