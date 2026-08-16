mod backend;
#[cfg(test)]
mod tests;

use loopal_protocol::WorkflowRunId;

use self::backend::AppendOutput;
use super::error::{WorkflowJournalError, WorkflowJournalLimit};
use super::record::WorkflowJournalEntry;
use super::validate;
use super::{MAX_WORKFLOW_JOURNAL_LINE_BYTES, MAX_WORKFLOW_JOURNAL_TOTAL_BYTES};

pub(crate) fn prepare(
    session_id: &str,
    run_id: &WorkflowRunId,
    entry: &WorkflowJournalEntry,
) -> Result<Vec<u8>, WorkflowJournalError> {
    validate::entry(session_id, run_id, entry)?;
    let mut line = serde_json::to_vec(entry)
        .map_err(|error| WorkflowJournalError::Serialization(error.to_string()))?;
    if line.len() > MAX_WORKFLOW_JOURNAL_LINE_BYTES {
        return Err(WorkflowJournalError::limit(
            WorkflowJournalLimit::LineBytes,
            line.len(),
            MAX_WORKFLOW_JOURNAL_LINE_BYTES,
        ));
    }
    line.push(b'\n');
    Ok(line)
}

pub(crate) fn append_init(
    location: &super::fs::JournalLocation,
    line: &[u8],
) -> Result<(), WorkflowJournalError> {
    let path = location.display_path();
    let opened =
        super::fs::open(location, super::fs::OpenMode::AppendCreate)?.ok_or_else(|| {
            WorkflowJournalError::Corruption {
                path: path.to_path_buf(),
                offset: 0,
                detail: "workflow journal disappeared during initialization".into(),
            }
        })?;
    append_with(path, line, opened.file)
}

pub(crate) fn append_commit(
    path: &std::path::Path,
    line: &[u8],
    file: std::fs::File,
) -> Result<(), WorkflowJournalError> {
    append_with(path, line, file)
}

fn append_with<O: AppendOutput>(
    path: &std::path::Path,
    line: &[u8],
    mut output: O,
) -> Result<(), WorkflowJournalError> {
    let current = output
        .byte_len()
        .map_err(|error| WorkflowJournalError::io(path, error))?;
    let total = current.saturating_add(line.len() as u64);
    if total > MAX_WORKFLOW_JOURNAL_TOTAL_BYTES {
        return Err(WorkflowJournalError::LimitExceeded {
            limit: WorkflowJournalLimit::TotalBytes,
            actual: total,
            max: MAX_WORKFLOW_JOURNAL_TOTAL_BYTES,
        });
    }
    output
        .write_all(line)
        .map_err(|error| WorkflowJournalError::io(path, error))?;
    output
        .flush()
        .map_err(|error| WorkflowJournalError::io(path, error))?;
    output
        .sync_data()
        .map_err(|error| WorkflowJournalError::io(path, error))
}
