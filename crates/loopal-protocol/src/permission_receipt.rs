use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    PermissionActionDigest, PermissionIntent, PermissionIntentDigest, PermissionIntentSeed,
    PermissionSchemaDigest, WorkflowPermissionCausation,
};

const MAX_TOKEN_BYTES: usize = 128;
const MAX_ISSUANCE_BYTES: usize = 128;

/// Hub-issued, effect-bound authorization. A receipt is scoped to one exact
/// action and one workflow attempt; it is consumed atomically at the effect
/// boundary and cannot be replayed after serialization or cloning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionReceipt {
    pub(crate) action_digest: PermissionActionDigest,
    pub(crate) schema_digest: PermissionSchemaDigest,
    pub(crate) intent_digest: PermissionIntentDigest,
    pub(crate) execution_generation: u64,
    pub(crate) ui_generation: u64,
    pub(crate) interaction_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) workflow: Option<WorkflowPermissionCausation>,
    pub(crate) audit_issuance: String,
}

impl PermissionReceipt {
    /// Create the wire authorization returned by the Hub after an allow
    /// decision. This is intentionally the only public constructor used by
    /// Hub code; callers cannot choose the receipt's binding fields.
    pub fn issue_for_intent(
        intent: &PermissionIntent,
        audit_issuance: impl Into<String>,
    ) -> Result<Self, PermissionReceiptError> {
        Self::issue(intent, audit_issuance)
    }

    pub fn issue(
        intent: &PermissionIntent,
        audit_issuance: impl Into<String>,
    ) -> Result<Self, PermissionReceiptError> {
        let receipt = Self {
            action_digest: intent.seed().action_digest(),
            schema_digest: intent.seed().schema_digest(),
            intent_digest: intent.intent_digest(),
            execution_generation: intent.execution_generation(),
            ui_generation: intent.ui_generation(),
            interaction_token: intent.interaction_token().to_string(),
            workflow: intent.seed().workflow().cloned(),
            audit_issuance: audit_issuance.into(),
        };
        receipt.validate_fields()?;
        Ok(receipt)
    }

    /// Construct a receipt for an in-process policy decision. Generation `1`
    /// is a reserved local lease; Hub-issued UI receipts always carry the
    /// authenticated connection/UI generations instead.
    pub fn issue_local(
        seed: &PermissionIntentSeed,
        audit_issuance: impl Into<String>,
    ) -> Result<Self, PermissionReceiptError> {
        let token = format!("local:{}", uuid::Uuid::new_v4().simple());
        let intent = PermissionIntent::bind(seed.clone(), 1, 1, token)
            .map_err(|_| PermissionReceiptError::Binding)?;
        Self::issue(&intent, audit_issuance)
    }

    pub fn action_digest(&self) -> PermissionActionDigest {
        self.action_digest
    }
    pub fn schema_digest(&self) -> PermissionSchemaDigest {
        self.schema_digest
    }
    pub fn intent_digest(&self) -> PermissionIntentDigest {
        self.intent_digest
    }
    pub fn execution_generation(&self) -> u64 {
        self.execution_generation
    }
    pub fn ui_generation(&self) -> u64 {
        self.ui_generation
    }
    pub fn interaction_token(&self) -> &str {
        &self.interaction_token
    }
    pub fn workflow(&self) -> Option<&WorkflowPermissionCausation> {
        self.workflow.as_ref()
    }
    pub fn audit_issuance(&self) -> &str {
        &self.audit_issuance
    }

    pub fn validate_for(&self, seed: &PermissionIntentSeed) -> Result<(), PermissionReceiptError> {
        self.validate_fields()?;
        if self.action_digest != seed.action_digest()
            || self.schema_digest != seed.schema_digest()
            || self.workflow.as_ref() != seed.workflow()
        {
            return Err(PermissionReceiptError::Binding);
        }
        let intent = PermissionIntent::bind(
            seed.clone(),
            self.execution_generation,
            self.ui_generation,
            self.interaction_token.clone(),
        )
        .map_err(|_| PermissionReceiptError::Binding)?;
        if intent.intent_digest() != self.intent_digest {
            return Err(PermissionReceiptError::Binding);
        }
        Ok(())
    }

    /// Validate the fields that the authenticated Hub can observe at the
    /// protected effect boundary. The issuance registry is intentionally
    /// owned by the Hub; a wire receipt by itself is not authority.
    pub fn validate_effect_binding(
        &self,
        action_digest: PermissionActionDigest,
        schema_digest: PermissionSchemaDigest,
        execution_generation: u64,
        workflow: Option<&WorkflowPermissionCausation>,
    ) -> Result<(), PermissionReceiptError> {
        self.validate_fields()?;
        if self.action_digest != action_digest
            || self.schema_digest != schema_digest
            || self.execution_generation != execution_generation
            || self.workflow.as_ref() != workflow
        {
            return Err(PermissionReceiptError::Binding);
        }
        Ok(())
    }

    fn validate_fields(&self) -> Result<(), PermissionReceiptError> {
        if self.execution_generation == 0 || self.ui_generation == 0 {
            return Err(PermissionReceiptError::Generation);
        }
        if self.interaction_token.is_empty()
            || self.interaction_token.len() > MAX_TOKEN_BYTES
            || self.interaction_token.chars().any(char::is_control)
            || self.audit_issuance.is_empty()
            || self.audit_issuance.len() > MAX_ISSUANCE_BYTES
            || self.audit_issuance.chars().any(char::is_control)
        {
            return Err(PermissionReceiptError::Token);
        }
        if self
            .workflow
            .as_ref()
            .is_some_and(|value| !value.is_valid())
        {
            return Err(PermissionReceiptError::Binding);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionReceiptError {
    Binding,
    Generation,
    Token,
    Consumed,
    Registry,
}

impl fmt::Display for PermissionReceiptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Binding => "permission receipt binding mismatch",
            Self::Generation => "permission receipt generation is invalid",
            Self::Token => "permission receipt token is invalid",
            Self::Consumed => "permission receipt was already consumed",
            Self::Registry => "permission receipt registry unavailable",
        })
    }
}

impl std::error::Error for PermissionReceiptError {}
