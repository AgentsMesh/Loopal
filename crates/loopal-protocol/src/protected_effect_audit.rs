use serde::{Deserialize, Serialize};

use crate::{PermissionActionDigest, PermissionReceipt, PermissionSchemaDigest};

const MAX_TOOL_CALL_ID_BYTES: usize = 512;
const MAX_TOOL_NAME_BYTES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedEffectAuditRequest {
    tool_call_id: String,
    tool_name: String,
    action_digest: PermissionActionDigest,
    schema_digest: PermissionSchemaDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    receipt: Option<PermissionReceipt>,
}

impl ProtectedEffectAuditRequest {
    pub fn new(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        action_digest: PermissionActionDigest,
        schema_digest: PermissionSchemaDigest,
    ) -> Result<Self, ProtectedEffectAuditError> {
        let request = Self {
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            action_digest,
            schema_digest,
            receipt: None,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), ProtectedEffectAuditError> {
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

    pub fn receipt(&self) -> Option<&PermissionReceipt> {
        self.receipt.as_ref()
    }

    pub fn with_receipt(mut self, receipt: PermissionReceipt) -> Self {
        self.receipt = Some(receipt);
        self
    }

    pub fn into_receipt(self) -> Option<PermissionReceipt> {
        self.receipt
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedEffectAuditResponse {
    pub recorded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtectedEffectAuditError;

impl std::fmt::Display for ProtectedEffectAuditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .write_str("protected effect audit text must be non-empty and within its byte limit")
    }
}

impl std::error::Error for ProtectedEffectAuditError {}

fn validate_text(value: &str, max_bytes: usize) -> Result<(), ProtectedEffectAuditError> {
    if value.is_empty() || value.len() > max_bytes {
        Err(ProtectedEffectAuditError)
    } else {
        Ok(())
    }
}
