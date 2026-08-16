use super::super::error::{WorkflowJournalError, WorkflowJournalLimit};
use super::super::{
    MAX_WORKFLOW_JOURNAL_ENTRIES, WorkflowJournalAppendDecision, WorkflowJournalAppendKind,
};
use super::WorkflowJournalReplay;

pub(super) fn validate(
    path: &std::path::Path,
    replay: &WorkflowJournalReplay,
    kind: &WorkflowJournalAppendKind,
    first_revision: Option<u64>,
) -> Result<WorkflowJournalAppendDecision, WorkflowJournalError> {
    if replay.torn_tail.is_some() {
        return Err(WorkflowJournalError::Corruption {
            path: path.to_path_buf(),
            offset: replay.last_good_offset,
            detail: "torn tail must be repaired before append".into(),
        });
    }
    if let Some(actual) = first_revision {
        validate_revision(path, replay, actual)?;
    }
    if matches!(kind, WorkflowJournalAppendKind::Commit)
        && (!replay.delivery_intents.is_empty() || !replay.delivery_acks.is_empty())
    {
        return Err(WorkflowJournalError::Corruption {
            path: path.to_path_buf(),
            offset: replay.last_good_offset,
            detail: "workflow events cannot follow terminal delivery records".into(),
        });
    }
    if matches!(kind, WorkflowJournalAppendKind::Init) && replay.init.is_some() {
        return Ok(WorkflowJournalAppendDecision::AlreadyPresent);
    }
    if let WorkflowJournalAppendKind::DeliveryAck(delivery_id) = kind
        && replay.delivery_acks.contains(delivery_id)
    {
        return Ok(WorkflowJournalAppendDecision::AlreadyPresent);
    }
    if let WorkflowJournalAppendKind::DeliveryIntent(notification) = kind
        && let Some(existing) = replay
            .delivery_intents
            .iter()
            .find(|existing| existing.delivery_id == notification.delivery_id)
    {
        return if existing == notification {
            Ok(WorkflowJournalAppendDecision::AlreadyPresent)
        } else {
            Err(WorkflowJournalError::Corruption {
                path: path.to_path_buf(),
                offset: replay.last_good_offset,
                detail: "delivery intent identity conflicts with persisted payload".into(),
            })
        };
    }
    if let WorkflowJournalAppendKind::DeliveryIntent(_) = kind
        && !replay.delivery_intents.is_empty()
    {
        return Err(WorkflowJournalError::Corruption {
            path: path.to_path_buf(),
            offset: replay.last_good_offset,
            detail: "workflow journal already has a terminal delivery intent".into(),
        });
    }
    let entries = replay.entry_count();
    if entries >= MAX_WORKFLOW_JOURNAL_ENTRIES {
        return Err(WorkflowJournalError::limit(
            WorkflowJournalLimit::Entries,
            entries + 1,
            MAX_WORKFLOW_JOURNAL_ENTRIES,
        ));
    }
    match (kind, replay.init.is_some()) {
        (WorkflowJournalAppendKind::Init, false) => Ok(WorkflowJournalAppendDecision::Append),
        (WorkflowJournalAppendKind::Commit, true) => Ok(WorkflowJournalAppendDecision::Append),
        (WorkflowJournalAppendKind::DeliveryIntent(notification), true) => {
            validate_delivery_revision(path, replay, notification.delivery_id.terminal_revision)?;
            super::snapshot::validate_terminal_intent(
                path,
                replay,
                notification,
                replay.last_good_offset,
            )?;
            Ok(WorkflowJournalAppendDecision::Append)
        }
        (WorkflowJournalAppendKind::DeliveryAck(delivery_id), true) => {
            validate_delivery_revision(path, replay, delivery_id.terminal_revision)?;
            if !replay
                .delivery_intents
                .iter()
                .any(|notification| notification.delivery_id == *delivery_id)
            {
                return Err(WorkflowJournalError::Corruption {
                    path: path.to_path_buf(),
                    offset: replay.last_good_offset,
                    detail: "delivery acknowledgement requires a matching intent".into(),
                });
            }
            Ok(WorkflowJournalAppendDecision::Append)
        }
        (WorkflowJournalAppendKind::Commit, false)
        | (WorkflowJournalAppendKind::DeliveryIntent(_), false)
        | (WorkflowJournalAppendKind::DeliveryAck(_), false) => {
            Err(WorkflowJournalError::Corruption {
                path: path.to_path_buf(),
                offset: 0,
                detail: "append requires an initialized journal".into(),
            })
        }
        (WorkflowJournalAppendKind::Init, true) => unreachable!("handled above"),
    }
}

fn validate_delivery_revision(
    path: &std::path::Path,
    replay: &WorkflowJournalReplay,
    actual: u64,
) -> Result<(), WorkflowJournalError> {
    let expected = last_revision(replay);
    if actual == expected {
        Ok(())
    } else {
        Err(WorkflowJournalError::Corruption {
            path: path.to_path_buf(),
            offset: replay.last_good_offset,
            detail: format!(
                "delivery terminal revision mismatch: expected {expected}, found {actual}"
            ),
        })
    }
}

fn validate_revision(
    path: &std::path::Path,
    replay: &WorkflowJournalReplay,
    actual: u64,
) -> Result<(), WorkflowJournalError> {
    let previous = last_revision(replay);
    let expected = previous.saturating_add(1);
    if actual == expected {
        Ok(())
    } else {
        Err(WorkflowJournalError::Corruption {
            path: path.to_path_buf(),
            offset: replay.last_good_offset,
            detail: format!("commit event revision gap: expected {expected}, found {actual}"),
        })
    }
}

fn last_revision(replay: &WorkflowJournalReplay) -> u64 {
    replay
        .commits
        .iter()
        .rev()
        .find_map(|commit| commit.events.last())
        .or_else(|| replay.init.as_ref()?.events.last())
        .map_or(0, |event| event.revision)
}

#[cfg(test)]
#[path = "preflight_branch_tests.rs"]
mod tests;
