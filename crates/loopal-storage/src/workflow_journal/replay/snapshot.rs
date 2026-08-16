use std::convert::Infallible;
use std::path::Path;

use loopal_protocol::{
    WorkflowJsonValidator, WorkflowReduceOutcome, WorkflowRunSnapshot,
    WorkflowTerminalNotification, reduce_workflow_event,
};

use super::{WorkflowJournalReplay, corrupt};
use crate::workflow_journal::error::WorkflowJournalError;

pub(super) fn validate_terminal_intent(
    path: &Path,
    replay: &WorkflowJournalReplay,
    notification: &WorkflowTerminalNotification,
    offset: u64,
) -> Result<(), WorkflowJournalError> {
    let snapshot = current(replay, path, offset)?;
    if !snapshot.state.is_terminal() {
        return Err(corrupt(
            path,
            offset,
            "delivery intent requires the current workflow snapshot to be terminal",
        ));
    }
    if notification.delivery_id.terminal_revision != snapshot.revision {
        return Err(corrupt(
            path,
            offset,
            format!(
                "delivery intent snapshot revision mismatch: expected {}, found {}",
                snapshot.revision, notification.delivery_id.terminal_revision
            ),
        ));
    }
    if notification.state != snapshot.state {
        return Err(corrupt(
            path,
            offset,
            format!(
                "delivery intent snapshot state mismatch: expected {:?}, found {:?}",
                snapshot.state, notification.state
            ),
        ));
    }
    Ok(())
}

fn current(
    replay: &WorkflowJournalReplay,
    path: &Path,
    offset: u64,
) -> Result<WorkflowRunSnapshot, WorkflowJournalError> {
    let init = replay.init.as_ref().ok_or_else(|| {
        corrupt(
            path,
            offset,
            "delivery intent requires an initialized journal",
        )
    })?;
    let mut snapshot = init.snapshot.clone();
    for event in init
        .events
        .iter()
        .chain(replay.commits.iter().flat_map(|commit| &commit.events))
    {
        snapshot = match reduce_workflow_event(&snapshot, event, &ReplayJsonValidator) {
            Ok(WorkflowReduceOutcome::Applied(next)) => *next,
            Ok(WorkflowReduceOutcome::IgnoredStale { .. }) => {
                return Err(corrupt(
                    path,
                    offset,
                    "delivery intent snapshot replay encountered a stale event",
                ));
            }
            Err(error) => {
                return Err(corrupt(
                    path,
                    offset,
                    format!("delivery intent snapshot replay failed: {error:?}"),
                ));
            }
        };
    }
    Ok(snapshot)
}

struct ReplayJsonValidator;

impl WorkflowJsonValidator for ReplayJsonValidator {
    type Error = Infallible;

    fn validate(
        &self,
        _schema: &serde_json::Value,
        _value: &serde_json::Value,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
