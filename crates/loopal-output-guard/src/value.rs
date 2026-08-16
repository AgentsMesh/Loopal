use serde_json::{Map, Value};
use thiserror::Error;

use crate::redactor::deduplicate;
use crate::{OutputGuard, Redaction};

#[derive(Clone, PartialEq, Eq)]
pub struct GuardedJson {
    value: Value,
    encoded_bytes: usize,
}

impl GuardedJson {
    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }

    pub fn into_value(self) -> Value {
        self.value
    }
}

impl std::fmt::Debug for GuardedJson {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GuardedJson")
            .field("encoded_bytes", &self.encoded_bytes)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JsonGuardError {
    #[error("redacted JSON object keys collide")]
    RedactedKeyCollision,
    #[error("redacted JSON encoding is {actual_bytes} bytes; limit is {max_bytes} bytes")]
    EncodedByteLimitExceeded {
        actual_bytes: usize,
        max_bytes: usize,
    },
}

impl OutputGuard {
    pub fn guard_json(
        &self,
        value: &Value,
        max_encoded_bytes: usize,
    ) -> Result<Redaction<GuardedJson>, JsonGuardError> {
        let mut redacted = value.clone();
        let mut names = Vec::new();
        redact_strings(self, &mut redacted, &mut names)?;
        deduplicate(&mut names);
        let encoded_bytes = serde_json::to_vec(&redacted)
            .expect("serde_json::Value serialization cannot fail")
            .len();
        if encoded_bytes > max_encoded_bytes {
            return Err(JsonGuardError::EncodedByteLimitExceeded {
                actual_bytes: encoded_bytes,
                max_bytes: max_encoded_bytes,
            });
        }
        Ok(Redaction::new(
            GuardedJson {
                value: redacted,
                encoded_bytes,
            },
            names,
        ))
    }
}

fn redact_strings(
    guard: &OutputGuard,
    value: &mut Value,
    names: &mut Vec<String>,
) -> Result<(), JsonGuardError> {
    match value {
        Value::String(text) => {
            let redacted = guard.redact_text(text);
            let (text, mut hits) = redacted.into_parts();
            *value = Value::String(text);
            names.append(&mut hits);
        }
        Value::Array(values) => {
            for value in values {
                redact_strings(guard, value, names)?;
            }
        }
        Value::Object(values) => redact_object(guard, values, names)?,
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn redact_object(
    guard: &OutputGuard,
    values: &mut Map<String, Value>,
    names: &mut Vec<String>,
) -> Result<(), JsonGuardError> {
    let mut redacted = Map::new();
    for (key, mut value) in std::mem::take(values) {
        let guarded_key = guard.redact_text(&key);
        let (key, mut hits) = guarded_key.into_parts();
        names.append(&mut hits);
        redact_strings(guard, &mut value, names)?;
        if redacted.insert(key, value).is_some() {
            return Err(JsonGuardError::RedactedKeyCollision);
        }
    }
    *values = redacted;
    Ok(())
}
