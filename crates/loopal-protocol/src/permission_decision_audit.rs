use serde::{Deserialize, Serialize};

use crate::{
    PermissionActionDigest, PermissionIntentDigest, PermissionIntentSeed, PermissionSchemaDigest,
};

const MAX_TOOL_CALL_ID_BYTES: usize = 256;
const MAX_TOOL_NAME_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAuditDecision {
    Allow,
    Deny,
}

impl PermissionAuditDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAuditSource {
    Frontend,
    Policy,
    RememberedGrant,
    Ui,
}

impl PermissionAuditSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Frontend => "frontend",
            Self::Policy => "policy",
            Self::RememberedGrant => "remembered_grant",
            Self::Ui => "ui",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionDecisionAuditRequest {
    tool_call_id: String,
    tool_name: String,
    action_digest: PermissionActionDigest,
    schema_digest: PermissionSchemaDigest,
    intent_digest: Option<PermissionIntentDigest>,
    decision: PermissionAuditDecision,
    source: PermissionAuditSource,
}

impl PermissionDecisionAuditRequest {
    pub fn from_seed(
        tool_call_id: impl Into<String>,
        seed: &PermissionIntentSeed,
        intent_digest: Option<PermissionIntentDigest>,
        decision: PermissionAuditDecision,
        source: PermissionAuditSource,
    ) -> Result<Self, PermissionDecisionAuditError> {
        Self::new(
            tool_call_id,
            seed.tool_name(),
            seed.action_digest(),
            seed.schema_digest(),
            intent_digest,
            decision,
            source,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        action_digest: PermissionActionDigest,
        schema_digest: PermissionSchemaDigest,
        intent_digest: Option<PermissionIntentDigest>,
        decision: PermissionAuditDecision,
        source: PermissionAuditSource,
    ) -> Result<Self, PermissionDecisionAuditError> {
        let request = Self {
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            action_digest,
            schema_digest,
            intent_digest,
            decision,
            source,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), PermissionDecisionAuditError> {
        validate_text(&self.tool_call_id, MAX_TOOL_CALL_ID_BYTES)?;
        validate_text(&self.tool_name, MAX_TOOL_NAME_BYTES)
    }

    pub fn tool_call_id(&self) -> &str {
        &self.tool_call_id
    }

    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    pub fn action_digest(&self) -> PermissionActionDigest {
        self.action_digest
    }

    pub fn schema_digest(&self) -> PermissionSchemaDigest {
        self.schema_digest
    }

    pub fn intent_digest(&self) -> Option<PermissionIntentDigest> {
        self.intent_digest
    }

    pub fn decision(&self) -> PermissionAuditDecision {
        self.decision
    }

    pub fn source(&self) -> PermissionAuditSource {
        self.source
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionDecisionAuditResponse {
    pub recorded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermissionDecisionAuditError;

impl std::fmt::Display for PermissionDecisionAuditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("permission audit text must be non-empty and within its byte limit")
    }
}

impl std::error::Error for PermissionDecisionAuditError {}

fn validate_text(value: &str, max_bytes: usize) -> Result<(), PermissionDecisionAuditError> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        Err(PermissionDecisionAuditError)
    } else {
        Ok(())
    }
}
