//! Cron snapshot state updates from `CronsChanged` / `SessionResumed` events.

use loopal_protocol::{AgentEventPayload, CronJobSnapshot};

use crate::state::SessionState;

pub(crate) fn apply(state: &mut SessionState, payload: AgentEventPayload) {
    match payload {
        AgentEventPayload::CronsChanged { crons } => {
            state.cron_snapshots = crons;
        }
        AgentEventPayload::SessionResumed { .. } => {
            // Clear stale cron snapshots from the prior session; the bridge
            // will re-emit on its next tick with the resumed session's
            // scheduler contents.
            state.cron_snapshots.clear();
        }
        _ => {}
    }
}

/// Whether any cron jobs are currently scheduled.
pub fn has_scheduled_crons(snapshots: &[CronJobSnapshot]) -> bool {
    !snapshots.is_empty()
}
