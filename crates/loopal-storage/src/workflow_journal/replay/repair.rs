use super::super::error::WorkflowJournalError;
use super::TornTail;

pub(super) fn run(
    location: &super::super::fs::JournalLocation,
    tail: TornTail,
) -> Result<(), WorkflowJournalError> {
    let path = location.display_path();
    if tail.path != path {
        return Err(mismatch(path));
    }
    let opened = super::super::fs::open(location, super::super::fs::OpenMode::Repair)?
        .ok_or_else(|| mismatch(path))?;
    if !super::super::fs::same_identity(&tail.identity, &opened.identity) {
        return Err(mismatch(path));
    }
    let file = opened.file;
    let len = file
        .metadata()
        .map_err(|error| WorkflowJournalError::io(path, error))?
        .len();
    if len != tail.observed_len || len <= tail.good_offset {
        return Err(mismatch(path));
    }
    file.set_len(tail.good_offset)
        .map_err(|error| WorkflowJournalError::io(path, error))?;
    file.sync_data()
        .map_err(|error| WorkflowJournalError::io(path, error))
}

fn mismatch(path: &std::path::Path) -> WorkflowJournalError {
    WorkflowJournalError::RepairMismatch {
        path: path.to_path_buf(),
    }
}

#[cfg(test)]
#[path = "repair_branch_tests.rs"]
mod tests;
