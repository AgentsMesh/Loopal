use std::path::PathBuf;

use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowJournalLimit {
    LineBytes,
    TotalBytes,
    SessionBytes,
    Journals,
    Entries,
    EventsPerCommit,
    RequestBytes,
}

#[derive(Debug, Error)]
pub enum WorkflowJournalError {
    #[error(transparent)]
    Storage(#[from] loopal_error::StorageError),
    #[error("invalid workflow run id: {0}")]
    InvalidRunId(String),
    #[error("workflow journal run id mismatch: expected {expected}, found {actual}")]
    RunIdMismatch { expected: String, actual: String },
    #[error("workflow journal limit {limit:?} exceeded: actual {actual}, max {max}")]
    LimitExceeded {
        limit: WorkflowJournalLimit,
        actual: u64,
        max: u64,
    },
    #[error("workflow journal at {path} is corrupt at byte {offset}: {detail}")]
    Corruption {
        path: PathBuf,
        offset: u64,
        detail: String,
    },
    #[error("workflow journal serialization failed: {0}")]
    Serialization(String),
    #[error("workflow journal I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("torn tail repair does not match {path}")]
    RepairMismatch { path: PathBuf },
}

impl WorkflowJournalError {
    pub(crate) fn io(path: &std::path::Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn limit(limit: WorkflowJournalLimit, actual: usize, max: usize) -> Self {
        Self::LimitExceeded {
            limit,
            actual: actual as u64,
            max: max as u64,
        }
    }
}
