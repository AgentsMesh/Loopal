use std::fmt;

use serde::{Deserialize, Serialize};

use crate::permission_digest::{
    PermissionActionDigest, PermissionDisplayDigest, PermissionIntentDigest,
    PermissionSchemaDigest, framed_sha256,
};
use crate::{WorkflowAttemptId, WorkflowNodeId, WorkflowRunId};

pub const PERMISSION_INTENT_VERSION: u8 = 2;
const MAX_TOOL_NAME_BYTES: usize = 256;
const MAX_INTERACTION_TOKEN_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPermissionCausation {
    pub run_id: WorkflowRunId,
    pub node_id: WorkflowNodeId,
    pub attempt_id: WorkflowAttemptId,
}

impl WorkflowPermissionCausation {
    pub fn is_valid(&self) -> bool {
        self.run_id.is_valid() && self.node_id.is_valid() && self.attempt_id.is_valid()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "PermissionIntentSeedWire",
    into = "PermissionIntentSeedWire"
)]
pub struct PermissionIntentSeed {
    version: u8,
    tool_name: String,
    action_digest: PermissionActionDigest,
    display_digest: PermissionDisplayDigest,
    schema_digest: PermissionSchemaDigest,
    workflow: Option<WorkflowPermissionCausation>,
}

impl PermissionIntentSeed {
    pub fn new(
        tool_name: impl Into<String>,
        action_digest: PermissionActionDigest,
        display_digest: PermissionDisplayDigest,
        schema_digest: PermissionSchemaDigest,
        workflow: Option<WorkflowPermissionCausation>,
    ) -> Result<Self, PermissionIntentError> {
        let seed = Self {
            version: PERMISSION_INTENT_VERSION,
            tool_name: tool_name.into(),
            action_digest,
            display_digest,
            schema_digest,
            workflow,
        };
        seed.validate()?;
        Ok(seed)
    }

    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    pub fn action_digest(&self) -> PermissionActionDigest {
        self.action_digest
    }

    pub fn display_digest(&self) -> PermissionDisplayDigest {
        self.display_digest
    }

    pub fn schema_digest(&self) -> PermissionSchemaDigest {
        self.schema_digest
    }

    pub fn workflow(&self) -> Option<&WorkflowPermissionCausation> {
        self.workflow.as_ref()
    }

    fn validate(&self) -> Result<(), PermissionIntentError> {
        if self.version != PERMISSION_INTENT_VERSION {
            return Err(PermissionIntentError::Version);
        }
        if self.tool_name.is_empty()
            || self.tool_name.len() > MAX_TOOL_NAME_BYTES
            || self.tool_name.chars().any(char::is_control)
        {
            return Err(PermissionIntentError::ToolName);
        }
        if self
            .workflow
            .as_ref()
            .is_some_and(|value| !value.is_valid())
        {
            return Err(PermissionIntentError::Workflow);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PermissionIntentWire", into = "PermissionIntentWire")]
pub struct PermissionIntent {
    seed: PermissionIntentSeed,
    execution_generation: u64,
    ui_generation: u64,
    interaction_token: String,
    intent_digest: PermissionIntentDigest,
}

impl PermissionIntent {
    pub fn bind(
        seed: PermissionIntentSeed,
        execution_generation: u64,
        ui_generation: u64,
        interaction_token: impl Into<String>,
    ) -> Result<Self, PermissionIntentError> {
        seed.validate()?;
        let interaction_token = interaction_token.into();
        validate_binding(execution_generation, ui_generation, &interaction_token)?;
        let intent_digest = calculate_digest(
            &seed,
            execution_generation,
            ui_generation,
            &interaction_token,
        );
        Ok(Self {
            seed,
            execution_generation,
            ui_generation,
            interaction_token,
            intent_digest,
        })
    }

    pub fn seed(&self) -> &PermissionIntentSeed {
        &self.seed
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

    pub fn intent_digest(&self) -> PermissionIntentDigest {
        self.intent_digest
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionIntentError {
    Version,
    ToolName,
    Workflow,
    Generation,
    InteractionToken,
    DigestMismatch,
}

impl fmt::Display for PermissionIntentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Version => "unsupported permission intent version",
            Self::ToolName => "invalid permission intent tool name",
            Self::Workflow => "invalid workflow permission causation",
            Self::Generation => "permission intent generations must be non-zero",
            Self::InteractionToken => "invalid permission interaction token",
            Self::DigestMismatch => "permission intent digest mismatch",
        })
    }
}

impl std::error::Error for PermissionIntentError {}

include!("permission_intent_wire.rs");
