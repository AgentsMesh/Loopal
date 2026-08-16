use std::path::Path;

use super::super::error::WorkflowJournalError;
use super::super::record::WorkflowJournalEntry;

pub(super) fn validate(
    entry: &WorkflowJournalEntry,
    last_revision: &mut u64,
    path: &Path,
    offset: u64,
) -> Result<(), WorkflowJournalError> {
    let events: &[_] = match entry {
        WorkflowJournalEntry::Init { events, .. } | WorkflowJournalEntry::Commit { events, .. } => {
            events
        }
        WorkflowJournalEntry::DeliveryIntent { notification, .. } => {
            validate_terminal_revision(
                notification.delivery_id.terminal_revision,
                *last_revision,
                path,
                offset,
            )?;
            &[]
        }
        WorkflowJournalEntry::DeliveryAck { delivery_id, .. } => {
            validate_terminal_revision(
                delivery_id.terminal_revision,
                *last_revision,
                path,
                offset,
            )?;
            &[]
        }
    };
    for event in events {
        let expected = last_revision.saturating_add(1);
        if event.revision != expected {
            return Err(WorkflowJournalError::Corruption {
                path: path.to_path_buf(),
                offset,
                detail: format!(
                    "event revision gap: expected {expected}, found {}",
                    event.revision
                ),
            });
        }
        *last_revision = event.revision;
    }
    Ok(())
}

fn validate_terminal_revision(
    actual: u64,
    expected: u64,
    path: &Path,
    offset: u64,
) -> Result<(), WorkflowJournalError> {
    if actual != expected {
        return Err(WorkflowJournalError::Corruption {
            path: path.to_path_buf(),
            offset,
            detail: format!(
                "delivery terminal revision mismatch: expected {}, found {}",
                expected, actual
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "sequence_tests.rs"]
mod tests;
