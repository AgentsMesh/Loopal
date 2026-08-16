use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{WorkflowFailureClass, WorkflowRunId, WorkflowRunState};

pub const DEFAULT_WORKFLOW_TERMINAL_APPLICATION_TIMEOUT: Duration = Duration::from_secs(3);
pub const DEFAULT_WORKFLOW_TERMINAL_RPC_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowTerminalDeliveryId {
    pub session_id: String,
    pub run_id: WorkflowRunId,
    pub terminal_revision: u64,
}

impl WorkflowTerminalDeliveryId {
    pub fn new(
        session_id: impl Into<String>,
        run_id: WorkflowRunId,
        terminal_revision: u64,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            run_id,
            terminal_revision,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowTerminalOutcome {
    Succeeded {
        result: String,
    },
    Failed {
        class: WorkflowFailureClass,
        reason: String,
    },
    Cancelled {
        reason: String,
    },
}

impl WorkflowTerminalOutcome {
    pub fn state(&self) -> WorkflowRunState {
        match self {
            Self::Succeeded { .. } => WorkflowRunState::Succeeded,
            Self::Failed { .. } => WorkflowRunState::Failed,
            Self::Cancelled { .. } => WorkflowRunState::Cancelled,
        }
    }

    pub(crate) fn detail(&self) -> &str {
        match self {
            Self::Succeeded { result } => result,
            Self::Failed { reason, .. } | Self::Cancelled { reason } => reason,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowTerminalNotification {
    pub delivery_id: WorkflowTerminalDeliveryId,
    pub state: WorkflowRunState,
    pub run_goal: String,
    pub outcome: WorkflowTerminalOutcome,
    pub content: String,
}

impl WorkflowTerminalNotification {
    /// Stable identity for the complete typed payload. These wire structs have
    /// no maps, so serde declaration order is canonical for this protocol.
    pub fn payload_digest(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("workflow terminal notification serializes");
        let mut hash = Sha256::new();
        hash.update(b"loopal.workflow-terminal.v1\0");
        hash.update(encoded);
        let mut digest = String::from("sha256:");
        for byte in hash.finalize() {
            use std::fmt::Write as _;
            write!(digest, "{byte:02x}").expect("writing to String cannot fail");
        }
        digest
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowTerminalDisposition {
    Applied,
    AlreadyApplied,
    Queued,
    Retryable { reason: String },
    Rejected { reason: String },
}
