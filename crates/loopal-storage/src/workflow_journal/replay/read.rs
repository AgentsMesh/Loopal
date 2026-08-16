use std::io::{BufReader, Seek, SeekFrom};
use std::path::Path;

use loopal_protocol::WorkflowRunId;

use super::super::error::{WorkflowJournalError, WorkflowJournalLimit};
use super::super::record::{WorkflowJournalCommit, WorkflowJournalEntry, WorkflowJournalInit};
use super::super::validate;
use super::super::{
    MAX_WORKFLOW_JOURNAL_ENTRIES, MAX_WORKFLOW_JOURNAL_LINE_BYTES, MAX_WORKFLOW_JOURNAL_TOTAL_BYTES,
};
use super::{TornTail, WorkflowJournalReplay, corrupt, sequence};

pub(super) fn opened(
    path: &Path,
    session_id: &str,
    run_id: &WorkflowRunId,
    file: &mut std::fs::File,
    identity: super::super::fs::FileIdentity,
) -> Result<WorkflowJournalReplay, WorkflowJournalError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| WorkflowJournalError::io(path, error))?;
    let total = file
        .metadata()
        .map_err(|error| WorkflowJournalError::io(path, error))?
        .len();
    enforce_total(total)?;
    let mut reader = BufReader::new(file);
    let mut replay = WorkflowJournalReplay::default();
    let mut line = Vec::new();
    let mut offset = 0u64;
    let mut last_revision = 0u64;
    loop {
        let consumed = super::super::io::read_bounded_line(
            &mut reader,
            &mut line,
            MAX_WORKFLOW_JOURNAL_LINE_BYTES,
        )
        .map_err(|error| WorkflowJournalError::io(path, error))?;
        if consumed == 0 {
            break;
        }
        let line_start = offset;
        offset = offset.saturating_add(consumed as u64);
        let terminated = line.last() == Some(&b'\n');
        if line.len() > MAX_WORKFLOW_JOURNAL_LINE_BYTES + usize::from(terminated) {
            return Err(corrupt(path, line_start, "encoded line exceeds limit"));
        }
        if !terminated {
            replay.torn_tail = Some(TornTail {
                path: path.to_path_buf(),
                good_offset: replay.last_good_offset,
                observed_len: total,
                identity,
            });
            break;
        }
        let entries = replay.entry_count();
        if entries >= MAX_WORKFLOW_JOURNAL_ENTRIES {
            return Err(WorkflowJournalError::limit(
                WorkflowJournalLimit::Entries,
                entries + 1,
                MAX_WORKFLOW_JOURNAL_ENTRIES,
            ));
        }
        let strict: super::super::strict::StrictEntry =
            serde_json::from_slice(&line[..line.len() - 1])
                .map_err(|error| corrupt(path, line_start, error.to_string()))?;
        let entry = WorkflowJournalEntry::try_from(strict)
            .map_err(|error| contextualize(error, path, line_start))?;
        validate::entry(session_id, run_id, &entry)
            .map_err(|error| contextualize(error, path, line_start))?;
        sequence::validate(&entry, &mut last_revision, path, line_start)?;
        apply(&mut replay, entry, path, line_start)?;
        replay.last_good_offset = offset;
    }
    Ok(replay)
}

fn apply(
    replay: &mut WorkflowJournalReplay,
    entry: WorkflowJournalEntry,
    path: &Path,
    offset: u64,
) -> Result<(), WorkflowJournalError> {
    match entry {
        WorkflowJournalEntry::Init {
            snapshot,
            events,
            request,
            ..
        } if replay.entry_count() == 0 => {
            replay.init = Some(WorkflowJournalInit {
                snapshot: *snapshot,
                events,
                request,
            });
        }
        WorkflowJournalEntry::Commit {
            run_id,
            events,
            request,
            ..
        } if replay.init.is_some()
            && replay.delivery_intents.is_empty()
            && replay.delivery_acks.is_empty() =>
        {
            replay.commits.push(WorkflowJournalCommit {
                run_id,
                events,
                request,
            });
        }
        WorkflowJournalEntry::DeliveryIntent { notification, .. } if replay.init.is_some() => {
            if !replay.delivery_intents.is_empty() || !replay.delivery_acks.is_empty() {
                return Err(corrupt(path, offset, "duplicate or late delivery intent"));
            }
            super::snapshot::validate_terminal_intent(path, replay, &notification, offset)?;
            replay.delivery_intents.push(notification);
        }
        WorkflowJournalEntry::DeliveryAck { delivery_id, .. } if replay.init.is_some() => {
            if replay.delivery_acks.contains(&delivery_id)
                || !replay
                    .delivery_intents
                    .iter()
                    .any(|notification| notification.delivery_id == delivery_id)
            {
                return Err(corrupt(
                    path,
                    offset,
                    "delivery acknowledgement lacks one matching unacknowledged intent",
                ));
            }
            replay.delivery_acks.push(delivery_id);
        }
        WorkflowJournalEntry::Init { .. } => {
            return Err(corrupt(
                path,
                offset,
                "init must be the first and only init entry",
            ));
        }
        WorkflowJournalEntry::Commit { .. } => {
            return Err(corrupt(path, offset, "commit encountered before init"));
        }
        WorkflowJournalEntry::DeliveryIntent { .. } => {
            return Err(corrupt(
                path,
                offset,
                "delivery intent encountered before init",
            ));
        }
        WorkflowJournalEntry::DeliveryAck { .. } => {
            return Err(corrupt(
                path,
                offset,
                "delivery acknowledgement encountered before init",
            ));
        }
    }
    Ok(())
}

fn enforce_total(total: u64) -> Result<(), WorkflowJournalError> {
    if total > MAX_WORKFLOW_JOURNAL_TOTAL_BYTES {
        return Err(WorkflowJournalError::LimitExceeded {
            limit: WorkflowJournalLimit::TotalBytes,
            actual: total,
            max: MAX_WORKFLOW_JOURNAL_TOTAL_BYTES,
        });
    }
    Ok(())
}

fn contextualize(error: WorkflowJournalError, path: &Path, offset: u64) -> WorkflowJournalError {
    match error {
        WorkflowJournalError::Corruption { detail, .. } => corrupt(path, offset, detail),
        other => other,
    }
}

#[cfg(test)]
#[path = "read_apply_tests.rs"]
mod apply_tests;
#[cfg(test)]
#[path = "read_context_tests.rs"]
mod context_tests;
#[cfg(test)]
#[path = "read_test_support.rs"]
mod test_support;
