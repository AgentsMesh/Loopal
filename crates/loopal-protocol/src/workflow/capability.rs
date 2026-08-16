use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::permission_digest::{WorkflowAttemptCapabilityDigest, framed_sha256};

const CAPABILITY_BYTES: usize = 32;
const ENCODED_BYTES: usize = CAPABILITY_BYTES * 2;

/// Opaque bearer proof issued for exactly one workflow attempt.
///
/// The raw value is sent only to the worker. Durable workflow state stores the
/// one-way digest returned by [`Self::digest`].
#[derive(Clone, PartialEq, Eq)]
pub struct WorkflowAttemptCapability(String);

impl WorkflowAttemptCapability {
    pub fn generate() -> Self {
        Self(format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, WorkflowAttemptCapabilityError> {
        let value = value.into();
        if value.len() != ENCODED_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(WorkflowAttemptCapabilityError);
        }
        Ok(Self(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> WorkflowAttemptCapabilityDigest {
        WorkflowAttemptCapabilityDigest::from_bytes(framed_sha256(
            b"loopal.workflow-attempt-capability.v1",
            &[self.0.as_bytes()],
        ))
    }

    pub fn matches_digest(&self, expected: WorkflowAttemptCapabilityDigest) -> bool {
        let actual = self.digest();
        actual
            .as_bytes()
            .iter()
            .zip(expected.as_bytes())
            .fold(0u8, |difference, (left, right)| difference | (left ^ right))
            == 0
    }
}

impl fmt::Debug for WorkflowAttemptCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WorkflowAttemptCapability([REDACTED])")
    }
}

impl Serialize for WorkflowAttemptCapability {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WorkflowAttemptCapability {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkflowAttemptCapabilityError;

impl fmt::Display for WorkflowAttemptCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("workflow attempt capability must be 64 lowercase hex digits")
    }
}

impl std::error::Error for WorkflowAttemptCapabilityError {}
