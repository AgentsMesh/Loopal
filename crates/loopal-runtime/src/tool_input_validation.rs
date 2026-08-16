use loopal_tool_api::{Tool, input_normalize::strip_empty_optionals};
use serde_json::Value;

pub fn validate_tool_input(tool: &dyn Tool, input: &Value) -> Result<(), String> {
    let schema = tool.parameters_schema();
    let mut normalized = input.clone();
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    strip_empty_optionals(&mut normalized, &required);
    validate_schema(&schema, &normalized, "$", &schema)
}

pub fn validate_wire_refs(input: &Value, eligible_fields: &[&str]) -> Result<(), String> {
    walk_wire_refs(input, eligible_fields, false)
}

fn validate_schema(schema: &Value, value: &Value, path: &str, root: &Value) -> Result<(), String> {
    if schema == &Value::Bool(true) || schema.is_null() {
        return Ok(());
    }
    if schema == &Value::Bool(false) {
        return Err(format!("{path} is not allowed by the tool schema"));
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let pointer = reference.strip_prefix('#').ok_or_else(|| {
            format!("{path} uses unsupported external schema reference {reference}")
        })?;
        let resolved = root
            .pointer(pointer)
            .ok_or_else(|| format!("{path} uses unresolved schema reference {reference}"))?;
        return validate_schema(resolved, value, path, root);
    }
    if let Some(expected) = schema.get("const")
        && value != expected
    {
        return Err(format!("{path} does not match the required constant"));
    }
    if let Some(options) = schema.get("enum").and_then(Value::as_array)
        && !options.contains(value)
    {
        return Err(format!("{path} is not one of the allowed values"));
    }
    validate_compositions(schema, value, path, root)?;
    validate_type(schema.get("type"), value, path)?;

    if let Some(object) = value.as_object() {
        let properties = schema.get("properties").and_then(Value::as_object);
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    return Err(format!("{path}.{key} is required"));
                }
            }
        }
        for (key, child) in object {
            if let Some(child_schema) = properties.and_then(|items| items.get(key)) {
                validate_schema(child_schema, child, &format!("{path}.{key}"), root)?;
            } else if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                return Err(format!("{path}.{key} is not allowed"));
            } else if let Some(extra_schema) = schema
                .get("additionalProperties")
                .filter(|item| item.is_object())
            {
                validate_schema(extra_schema, child, &format!("{path}.{key}"), root)?;
            }
        }
    }
    if let (Some(values), Some(items)) = (value.as_array(), schema.get("items")) {
        for (index, child) in values.iter().enumerate() {
            validate_schema(items, child, &format!("{path}[{index}]"), root)?;
        }
    }
    Ok(())
}

fn validate_compositions(
    schema: &Value,
    value: &Value,
    path: &str,
    root: &Value,
) -> Result<(), String> {
    if let Some(items) = schema.get("allOf").and_then(Value::as_array) {
        for item in items {
            validate_schema(item, value, path, root)?;
        }
    }
    if let Some(items) = schema.get("anyOf").and_then(Value::as_array)
        && !items
            .iter()
            .any(|item| validate_schema(item, value, path, root).is_ok())
    {
        return Err(format!("{path} does not match any allowed schema"));
    }
    if let Some(items) = schema.get("oneOf").and_then(Value::as_array)
        && items
            .iter()
            .filter(|item| validate_schema(item, value, path, root).is_ok())
            .count()
            != 1
    {
        return Err(format!("{path} does not match exactly one allowed schema"));
    }
    Ok(())
}

fn validate_type(schema_type: Option<&Value>, value: &Value, path: &str) -> Result<(), String> {
    let Some(schema_type) = schema_type else {
        return Ok(());
    };
    let matches = match schema_type {
        Value::String(kind) => type_matches(kind, value),
        Value::Array(kinds) => kinds
            .iter()
            .filter_map(Value::as_str)
            .any(|kind| type_matches(kind, value)),
        _ => true,
    };
    if matches {
        Ok(())
    } else {
        Err(format!("{path} has the wrong JSON type"))
    }
}

fn type_matches(kind: &str, value: &Value) -> bool {
    match kind {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "string" => value.is_string(),
        _ => true,
    }
}

fn walk_wire_refs(value: &Value, eligible: &[&str], in_eligible: bool) -> Result<(), String> {
    match value {
        Value::Object(values) => {
            for (key, child) in values {
                walk_wire_refs(
                    child,
                    eligible,
                    in_eligible || eligible.contains(&key.as_str()),
                )?;
            }
        }
        Value::Array(values) => {
            for child in values {
                walk_wire_refs(child, eligible, in_eligible)?;
            }
        }
        Value::String(text) => validate_wire_string(text, in_eligible)?,
        _ => {}
    }
    Ok(())
}

fn validate_wire_string(text: &str, eligible: bool) -> Result<(), String> {
    let mut offset = 0;
    while let Some(relative) = text[offset..].find("<secret_ref:") {
        let start = offset + relative;
        let Some(found) = loopal_secret_client::WIRE_RE.find_at(text, start) else {
            return Err("malformed <secret_ref:NAME> placeholder".into());
        };
        if found.start() != start {
            return Err("malformed <secret_ref:NAME> placeholder".into());
        }
        if !eligible {
            return Err("secret placeholder appears in a non-secret-eligible parameter".into());
        }
        offset = found.end();
    }
    Ok(())
}

#[cfg(test)]
#[path = "tool_input_validation/tests.rs"]
mod tests;
