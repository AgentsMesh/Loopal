use serde_json::Value;

pub(super) fn required_nonempty_string(params: &Value, field: &str) -> Result<String, String> {
    optional_nonempty_string(params, field)?.ok_or_else(|| format!("missing '{field}' field"))
}

pub(super) fn optional_nonempty_string(
    params: &Value,
    field: &str,
) -> Result<Option<String>, String> {
    match params.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.trim().into())),
        Some(_) => Err(format!("'{field}' must be a non-empty string")),
    }
}

pub(super) fn optional_string(params: &Value, field: &str) -> Result<Option<String>, String> {
    match params.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("'{field}' must be a string")),
    }
}

pub(super) fn require_optional_string(
    params: &Value,
    field: &str,
    expected: &str,
) -> Result<(), String> {
    match params.get(field) {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(value)) if value == expected => Ok(()),
        Some(_) => Err(format!("'{field}' conflicts with authenticated authority")),
    }
}

pub(super) fn require_optional_bool(
    params: &Value,
    field: &str,
    expected: bool,
) -> Result<(), String> {
    match params.get(field) {
        None | Some(Value::Null) => Ok(()),
        Some(Value::Bool(value)) if *value == expected => Ok(()),
        Some(_) => Err(format!("'{field}' conflicts with authenticated authority")),
    }
}

pub(super) fn require_optional_u32(
    params: &Value,
    field: &str,
    expected: u32,
) -> Result<(), String> {
    match params.get(field) {
        None | Some(Value::Null) => Ok(()),
        Some(Value::Number(value)) if value.as_u64() == Some(u64::from(expected)) => Ok(()),
        Some(_) => Err(format!("'{field}' conflicts with authenticated authority")),
    }
}

pub(super) fn reject_fields(params: &Value, fields: &[&str]) -> Result<(), String> {
    for field in fields {
        if params.get(field).is_some_and(|value| !value.is_null()) {
            return Err(format!("'{field}' is Hub-derived and cannot be supplied"));
        }
    }
    Ok(())
}
