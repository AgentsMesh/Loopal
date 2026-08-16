use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use loopal_protocol::{WorkflowEvent, WorkflowEventPayload};

use super::super::error::{WorkflowJournalError, WorkflowJournalLimit};
use super::super::record::WorkflowJournalEntry;
use super::super::{MAX_WORKFLOW_JOURNAL_LINE_BYTES, MAX_WORKFLOW_JOURNAL_TOTAL_BYTES};
use super::backend::AppendOutput;
use super::{append_with, prepare};

#[derive(Clone, Copy)]
enum Failure {
    Length,
    Write,
    Flush,
    Sync,
}

struct FailingWriter {
    failure: Failure,
    length: u64,
    bytes: Arc<Mutex<Vec<u8>>>,
}

impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if matches!(self.failure, Failure::Write) {
            self.bytes.lock().unwrap().extend_from_slice(&bytes[..1]);
            return Err(io::Error::other("write failed"));
        }
        self.bytes.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.failure {
            Failure::Flush => Err(io::Error::other("flush failed")),
            _ => Ok(()),
        }
    }
}

impl AppendOutput for FailingWriter {
    fn byte_len(&self) -> io::Result<u64> {
        match self.failure {
            Failure::Length => Err(io::Error::other("length failed")),
            _ => Ok(self.length),
        }
    }

    fn sync_data(&self) -> io::Result<()> {
        match self.failure {
            Failure::Sync => Err(io::Error::other("sync failed")),
            _ => Ok(()),
        }
    }
}

#[test]
fn append_surfaces_length_write_flush_and_sync_failures() {
    for failure in [
        Failure::Length,
        Failure::Write,
        Failure::Flush,
        Failure::Sync,
    ] {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let writer = FailingWriter {
            failure,
            length: 0,
            bytes: bytes.clone(),
        };
        assert!(append_with(Path::new("journal.jsonl"), b"{}\n", writer).is_err());
        if matches!(failure, Failure::Write) {
            assert_ne!(*bytes.lock().unwrap(), b"{}\n");
        }
    }
}

#[test]
fn append_rejects_opened_file_over_total_limit_before_write() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = FailingWriter {
        failure: Failure::Sync,
        length: MAX_WORKFLOW_JOURNAL_TOTAL_BYTES,
        bytes: bytes.clone(),
    };
    assert!(matches!(
        append_with(Path::new("journal.jsonl"), b"x", writer),
        Err(WorkflowJournalError::LimitExceeded {
            limit: WorkflowJournalLimit::TotalBytes,
            ..
        })
    ));
    assert!(bytes.lock().unwrap().is_empty());
}

#[test]
fn prepare_rejects_encoded_line_over_limit() {
    let run_id = loopal_protocol::WorkflowRunId::from("wrun_test");
    let entry = WorkflowJournalEntry::commit(
        run_id.clone(),
        vec![WorkflowEvent {
            run_id: run_id.clone(),
            revision: 1,
            occurred_at_unix_ms: 1,
            payload: WorkflowEventPayload::CancelRequested {
                reason: Some("x".repeat(MAX_WORKFLOW_JOURNAL_LINE_BYTES)),
            },
        }],
        None,
    );
    assert!(matches!(
        prepare("session-one", &run_id, &entry),
        Err(WorkflowJournalError::LimitExceeded {
            limit: WorkflowJournalLimit::LineBytes,
            ..
        })
    ));
}
