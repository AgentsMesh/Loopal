use loopal_error::{ConfigError, LoopalError};

#[derive(Debug)]
pub enum LocalSettingsFieldPatch {
    Set(String, serde_json::Value),
    Remove(String),
    EnsureObject(String),
}

pub(super) fn apply(
    current: &mut serde_json::Value,
    fields: impl IntoIterator<Item = LocalSettingsFieldPatch>,
) -> Result<(), LoopalError> {
    if current.is_null() {
        *current = serde_json::Value::Object(serde_json::Map::new());
    }
    let obj = current
        .as_object_mut()
        .ok_or_else(|| ConfigError::InvalidValue {
            field: "settings.json".into(),
            reason: "top-level JSON is not an object".into(),
        })?;
    for field in fields {
        match field {
            LocalSettingsFieldPatch::Set(path, value) => insert_path(obj, &path, value)?,
            LocalSettingsFieldPatch::Remove(path) => remove_path(obj, &path)?,
            LocalSettingsFieldPatch::EnsureObject(path) => ensure_object(obj, &path)?,
        }
    }
    Ok(())
}

fn insert_path(
    root: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
    value: serde_json::Value,
) -> Result<(), LoopalError> {
    let mut segments = path.split('.').peekable();
    let mut current = root;
    while let Some(segment) = segments.next() {
        validate_segment(path, segment)?;
        if segments.peek().is_none() {
            current.insert(segment.into(), value);
            return Ok(());
        }
        let entry = current
            .entry(segment)
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if entry.is_null() {
            *entry = serde_json::Value::Object(serde_json::Map::new());
        }
        current = entry
            .as_object_mut()
            .ok_or_else(|| invalid_path(path, format!("'{segment}' is not an object")))?;
    }
    Ok(())
}

fn remove_path(
    root: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Result<(), LoopalError> {
    let segments: Vec<_> = path.split('.').collect();
    if segments.is_empty() || segments.iter().any(|segment| segment.is_empty()) {
        return Err(invalid_path(path, "setting path contains an empty segment"));
    }
    let mut current = root;
    for segment in &segments[..segments.len() - 1] {
        let Some(value) = current.get_mut(*segment) else {
            return Ok(());
        };
        let Some(next) = value.as_object_mut() else {
            return Ok(());
        };
        current = next;
    }
    current.remove(segments[segments.len() - 1]);
    Ok(())
}

fn ensure_object(
    root: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Result<(), LoopalError> {
    let mut current = root;
    for segment in path.split('.') {
        validate_segment(path, segment)?;
        let entry = current.entry(segment).or_insert(serde_json::Value::Null);
        if entry.is_null() {
            *entry = serde_json::Value::Object(serde_json::Map::new());
        }
        current = entry
            .as_object_mut()
            .ok_or_else(|| invalid_path(path, format!("'{segment}' is not an object")))?;
    }
    Ok(())
}

fn validate_segment(path: &str, segment: &str) -> Result<(), LoopalError> {
    if segment.is_empty() {
        return Err(invalid_path(path, "setting path contains an empty segment"));
    }
    Ok(())
}

fn invalid_path(path: &str, reason: impl Into<String>) -> LoopalError {
    ConfigError::InvalidValue {
        field: path.into(),
        reason: reason.into(),
    }
    .into()
}
