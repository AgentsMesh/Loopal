use std::time::Instant;

use crate::error::TransitionError;
use crate::invocation::ToolInvocation;
use crate::outcome::{CancelCause, Outcome, StaleReason};
use crate::progress::ProgressSnapshot;
use crate::state::InvocationState;

pub enum TransitionCmd {
    RecordProgress(ProgressSnapshot),
    Complete(Outcome),
    MarkStale(StaleReason),
    Cancel(CancelCause),
}

impl TransitionCmd {
    fn name(&self) -> &'static str {
        match self {
            Self::RecordProgress(_) => "RecordProgress",
            Self::Complete(_) => "Complete",
            Self::MarkStale(_) => "MarkStale",
            Self::Cancel(_) => "Cancel",
        }
    }
}

pub fn transition(
    invocation: ToolInvocation,
    cmd: TransitionCmd,
    now: Instant,
) -> Result<ToolInvocation, TransitionError> {
    let elapsed = now.saturating_duration_since(invocation.started_at);
    let next_state = step(&invocation.state, cmd, elapsed)?;
    Ok(ToolInvocation {
        state: next_state,
        ..invocation
    })
}

fn step(
    state: &InvocationState,
    cmd: TransitionCmd,
    elapsed: std::time::Duration,
) -> Result<InvocationState, TransitionError> {
    use InvocationState::*;
    match (state, cmd) {
        (Pending | Running { .. }, TransitionCmd::RecordProgress(p)) => Ok(Running {
            last_progress: Some(p),
        }),

        (Pending | Running { .. }, TransitionCmd::Complete(o)) => Ok(Done {
            duration: elapsed,
            outcome: o,
        }),

        (Pending | Running { .. }, TransitionCmd::MarkStale(r)) => Ok(Stale {
            duration: elapsed,
            reason: r,
        }),

        (Pending | Running { .. }, TransitionCmd::Cancel(c)) => Ok(Cancelled {
            duration: elapsed,
            cause: c,
        }),

        (Done { .. } | Stale { .. } | Cancelled { .. }, cmd) => Err(TransitionError::Invalid {
            state: state.variant_name(),
            cmd: cmd.name(),
        }),
    }
}
