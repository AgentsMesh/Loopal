use std::path::PathBuf;

use loopal_error::StorageError;
use loopal_protocol::WorkflowRunId;

use super::{SessionStore, validate_path_component};
use crate::workflow_journal::fs::{FileIdentity, discover};
use crate::{
    MAX_WORKFLOW_JOURNALS_PER_SESSION, MAX_WORKFLOW_SESSION_JOURNAL_BYTES, WorkflowJournalError,
    WorkflowJournalLimit,
};

pub(crate) struct DiscoveredWorkflowJournal {
    pub(crate) run_id: WorkflowRunId,
    pub(crate) identity: FileIdentity,
}

impl SessionStore {
    pub fn workflow_journal_path(
        &self,
        session_id: &str,
        run_id: &str,
    ) -> Result<PathBuf, StorageError> {
        validate_path_component("session_id", session_id)?;
        validate_path_component("run_id", run_id)?;
        Ok(self
            .session_dir(session_id)
            .join("workflows")
            .join(format!("{run_id}.jsonl")))
    }

    pub fn list_workflow_run_ids(
        &self,
        session_id: &str,
    ) -> Result<Vec<WorkflowRunId>, WorkflowJournalError> {
        self.discover_workflow_journals(session_id)
            .map(|journals| journals.into_iter().map(|journal| journal.run_id).collect())
    }

    pub fn list_workflow_journals(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::WorkflowJournal>, WorkflowJournalError> {
        self.discover_workflow_journals(session_id)?
            .into_iter()
            .map(|journal| {
                crate::WorkflowJournal::from_discovered(
                    self,
                    session_id,
                    journal.run_id,
                    journal.identity,
                )
            })
            .collect()
    }

    fn discover_workflow_journals(
        &self,
        session_id: &str,
    ) -> Result<Vec<DiscoveredWorkflowJournal>, WorkflowJournalError> {
        validate_path_component("session_id", session_id)?;
        let directory = self.session_dir(session_id).join("workflows");
        let entries = discover(self.base_dir(), session_id, &directory)?;
        let mut journals = Vec::with_capacity(entries.len());
        let mut total_bytes = 0u64;
        for entry in entries {
            let path = directory.join(&entry.name);
            let name = entry
                .name
                .into_string()
                .map_err(|_| corrupt(&path, "workflow journal filename is not UTF-8"))?;
            let stem = name
                .strip_suffix(".jsonl")
                .ok_or_else(|| corrupt(&path, "workflow journal filename must end in .jsonl"))?;
            let run_id = WorkflowRunId::new(stem);
            if !run_id.is_valid() {
                return Err(corrupt(
                    &path,
                    "workflow journal filename has an invalid run id",
                ));
            }
            total_bytes = total_bytes.saturating_add(entry.bytes);
            if total_bytes > MAX_WORKFLOW_SESSION_JOURNAL_BYTES {
                return Err(WorkflowJournalError::LimitExceeded {
                    limit: WorkflowJournalLimit::SessionBytes,
                    actual: total_bytes,
                    max: MAX_WORKFLOW_SESSION_JOURNAL_BYTES,
                });
            }
            journals.push(DiscoveredWorkflowJournal {
                run_id,
                identity: entry.identity,
            });
            if journals.len() > MAX_WORKFLOW_JOURNALS_PER_SESSION {
                return Err(WorkflowJournalError::limit(
                    WorkflowJournalLimit::Journals,
                    journals.len(),
                    MAX_WORKFLOW_JOURNALS_PER_SESSION,
                ));
            }
        }
        journals.sort_by(|left, right| left.run_id.cmp(&right.run_id));
        Ok(journals)
    }
}

fn corrupt(path: &std::path::Path, detail: &str) -> WorkflowJournalError {
    WorkflowJournalError::Corruption {
        path: path.to_path_buf(),
        offset: 0,
        detail: detail.into(),
    }
}
