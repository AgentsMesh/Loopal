use anyhow::{Result, ensure};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

const MAX_MATCHERS: usize = 64;
const MAX_PATH_CHARS: usize = 512;
const MAX_TEXT_CHARS: usize = 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RequestMetadataMatcher {
    path: String,
    exists: Option<bool>,
    #[serde(default)]
    equals: OptionalValue,
    contains: Option<String>,
    excludes: Option<String>,
}

#[derive(Clone, Debug, Default)]
enum OptionalValue {
    #[default]
    Missing,
    Present(Value),
}

impl<'de> Deserialize<'de> for OptionalValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(Self::Present)
    }
}

impl RequestMetadataMatcher {
    pub(crate) fn validate_all(matchers: &[Self]) -> Result<()> {
        ensure!(!matchers.is_empty(), "requestMetadata cannot be empty");
        ensure!(
            matchers.len() <= MAX_MATCHERS,
            "requestMetadata has too many matchers"
        );
        matchers.iter().try_for_each(Self::validate)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            valid_pointer(&self.path),
            "metadata path must be a JSON pointer"
        );
        ensure!(
            self.path.chars().count() <= MAX_PATH_CHARS,
            "metadata path is too long"
        );
        let has_value_predicate = !matches!(self.equals, OptionalValue::Missing)
            || self.contains.is_some()
            || self.excludes.is_some();
        ensure!(
            self.exists.is_some() || has_value_predicate,
            "metadata matcher requires a predicate"
        );
        ensure!(
            self.exists != Some(false) || !has_value_predicate,
            "exists false cannot be combined with value predicates"
        );
        for value in [&self.contains, &self.excludes].into_iter().flatten() {
            ensure!(!value.is_empty(), "metadata text predicate cannot be empty");
            ensure!(
                value.chars().count() <= MAX_TEXT_CHARS,
                "metadata text predicate is too long"
            );
        }
        Ok(())
    }

    pub(crate) fn mismatch(&self, metadata: Option<&Value>) -> Vec<String> {
        let value = metadata.and_then(|root| {
            if self.path.is_empty() {
                Some(root)
            } else {
                root.pointer(&self.path)
            }
        });
        let mut errors = Vec::new();
        if self
            .exists
            .is_some_and(|expected| expected != value.is_some())
        {
            errors.push(self.error("existence did not match"));
        }
        if let OptionalValue::Present(expected) = &self.equals
            && value != Some(expected)
        {
            errors.push(self.error("did not equal expected value"));
        }
        if let Some(expected) = &self.contains {
            match value.and_then(Value::as_str) {
                Some(actual) if actual.contains(expected) => {}
                Some(_) => errors.push(self.error("did not contain expected text")),
                None => errors.push(self.error("was not a string for contains")),
            }
        }
        if let Some(excluded) = &self.excludes {
            match value.and_then(Value::as_str) {
                Some(actual) if !actual.contains(excluded) => {}
                Some(_) => errors.push(self.error("contained excluded text")),
                None => errors.push(self.error("was not a string for excludes")),
            }
        }
        errors
    }

    fn error(&self, reason: &str) -> String {
        let path = if self.path.is_empty() {
            "<root>"
        } else {
            &self.path
        };
        format!("request metadata {path} {reason}")
    }
}

fn valid_pointer(path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    if !path.starts_with('/') || path.chars().any(char::is_control) {
        return false;
    }
    let mut chars = path.chars();
    while let Some(character) = chars.next() {
        if character == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}
