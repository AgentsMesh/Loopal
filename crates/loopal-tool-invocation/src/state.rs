use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::outcome::{CancelCause, Outcome, StaleReason};
use crate::progress::ProgressSnapshot;

/// Tool invocation lifecycle state.
///
/// Time semantics live on the wrapping `ToolInvocation`:
/// - `ToolInvocation.started_at: Instant` is the process-local start time
///   (`#[serde(skip)]`, defaults to `now` on deserialize).
/// - `InvocationState` itself carries only a `duration: Duration` on
///   terminal variants — the total runtime at the terminal transition,
///   preserved across the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum InvocationState {
    Pending,
    Running {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_progress: Option<ProgressSnapshot>,
    },
    Done {
        duration: Duration,
        outcome: Outcome,
    },
    Stale {
        duration: Duration,
        reason: StaleReason,
    },
    Cancelled {
        duration: Duration,
        cause: CancelCause,
    },
}

impl InvocationState {
    pub fn duration(&self) -> Option<Duration> {
        match self {
            Self::Pending | Self::Running { .. } => None,
            Self::Done { duration, .. }
            | Self::Stale { duration, .. }
            | Self::Cancelled { duration, .. } => Some(*duration),
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::Running { .. })
    }

    pub fn is_terminal(&self) -> bool {
        !self.is_active()
    }

    pub fn progress_tail(&self) -> Option<&str> {
        match self {
            Self::Running {
                last_progress: Some(p),
                ..
            } => Some(&p.tail),
            _ => None,
        }
    }

    pub fn outcome(&self) -> Option<&Outcome> {
        match self {
            Self::Done { outcome, .. } => Some(outcome),
            _ => None,
        }
    }

    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Running { .. } => "Running",
            Self::Done { .. } => "Done",
            Self::Stale { .. } => "Stale",
            Self::Cancelled { .. } => "Cancelled",
        }
    }
}
