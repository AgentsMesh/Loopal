use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct InvocationId(String);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InvocationIdError {
    #[error("invocation id cannot be empty")]
    Empty,
}

impl InvocationId {
    pub fn new(value: impl Into<String>) -> Result<Self, InvocationIdError> {
        let s = value.into();
        if s.is_empty() {
            return Err(InvocationIdError::Empty);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for InvocationId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InvocationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for InvocationId {
    type Error = InvocationIdError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for InvocationId {
    type Error = InvocationIdError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_owned())
    }
}

impl<'de> Deserialize<'de> for InvocationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}
