use std::fmt;

use super::{WorkflowTerminalDeliveryId, WorkflowTerminalNotification};

pub const MAX_WORKFLOW_TERMINAL_SESSION_ID_BYTES: usize = 128;
pub const MAX_WORKFLOW_TERMINAL_GOAL_BYTES: usize = 4 * 1_024;
pub const MAX_WORKFLOW_TERMINAL_DETAIL_BYTES: usize = 64 * 1_024;
pub const MAX_WORKFLOW_TERMINAL_CONTENT_BYTES: usize = 72 * 1_024;
pub const WORKFLOW_TERMINAL_TRUNCATION_MARKER: &str = "\n[workflow result truncated]";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowTerminalValidationError {
    InvalidSessionId,
    InvalidRunId,
    InvalidTerminalRevision,
    StateMismatch,
    TooLarge {
        field: &'static str,
        actual_bytes: usize,
        max_bytes: usize,
    },
}

impl fmt::Display for WorkflowTerminalValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSessionId => formatter.write_str("invalid workflow terminal session id"),
            Self::InvalidRunId => formatter.write_str("invalid workflow terminal run id"),
            Self::InvalidTerminalRevision => {
                formatter.write_str("workflow terminal revision must be greater than zero")
            }
            Self::StateMismatch => {
                formatter.write_str("workflow terminal state does not match outcome")
            }
            Self::TooLarge {
                field,
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "workflow terminal {field} is {actual_bytes} bytes; limit is {max_bytes}"
            ),
        }
    }
}

impl std::error::Error for WorkflowTerminalValidationError {}

impl WorkflowTerminalDeliveryId {
    pub fn validate(&self) -> Result<(), WorkflowTerminalValidationError> {
        let session_valid = !self.session_id.is_empty()
            && self.session_id.len() <= MAX_WORKFLOW_TERMINAL_SESSION_ID_BYTES
            && !matches!(self.session_id.as_str(), "." | "..")
            && !self.session_id.contains(['/', '\\']);
        if !session_valid {
            return Err(WorkflowTerminalValidationError::InvalidSessionId);
        }
        if !self.run_id.is_valid() {
            return Err(WorkflowTerminalValidationError::InvalidRunId);
        }
        if self.terminal_revision == 0 {
            return Err(WorkflowTerminalValidationError::InvalidTerminalRevision);
        }
        Ok(())
    }
}

impl WorkflowTerminalNotification {
    pub fn validate(&self) -> Result<(), WorkflowTerminalValidationError> {
        self.delivery_id.validate()?;
        if !self.state.is_terminal() || self.outcome.state() != self.state {
            return Err(WorkflowTerminalValidationError::StateMismatch);
        }
        validate_bound("run_goal", &self.run_goal, MAX_WORKFLOW_TERMINAL_GOAL_BYTES)?;
        validate_bound(
            "outcome",
            self.outcome.detail(),
            MAX_WORKFLOW_TERMINAL_DETAIL_BYTES,
        )?;
        validate_bound(
            "content",
            &self.content,
            MAX_WORKFLOW_TERMINAL_CONTENT_BYTES,
        )
    }
}

fn validate_bound(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), WorkflowTerminalValidationError> {
    if value.len() <= max_bytes {
        Ok(())
    } else {
        Err(WorkflowTerminalValidationError::TooLarge {
            field,
            actual_bytes: value.len(),
            max_bytes,
        })
    }
}

pub fn truncate_workflow_terminal_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let marker = truncate_at_char_boundary(WORKFLOW_TERMINAL_TRUNCATION_MARKER, max_bytes);
    let body_limit = max_bytes.saturating_sub(marker.len());
    let mut bounded = truncate_at_char_boundary(value, body_limit).to_string();
    bounded.push_str(marker);
    bounded
}

fn truncate_at_char_boundary(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}
