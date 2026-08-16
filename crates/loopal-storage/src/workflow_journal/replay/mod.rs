mod preflight;
mod read;
mod repair;
mod sequence;
mod snapshot;

use std::path::PathBuf;

use loopal_protocol::{WorkflowRunId, WorkflowTerminalDeliveryId, WorkflowTerminalNotification};

use super::error::WorkflowJournalError;
use super::record::{WorkflowJournalCommit, WorkflowJournalInit};
use super::{WorkflowJournalAppendDecision, WorkflowJournalAppendKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TornTail {
    path: PathBuf,
    good_offset: u64,
    observed_len: u64,
    identity: super::fs::FileIdentity,
}

impl TornTail {
    pub fn good_offset(&self) -> u64 {
        self.good_offset
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkflowJournalReplay {
    pub init: Option<WorkflowJournalInit>,
    pub commits: Vec<WorkflowJournalCommit>,
    pub delivery_intents: Vec<WorkflowTerminalNotification>,
    pub delivery_acks: Vec<WorkflowTerminalDeliveryId>,
    pub torn_tail: Option<TornTail>,
    pub last_good_offset: u64,
}

impl WorkflowJournalReplay {
    pub fn entry_count(&self) -> usize {
        usize::from(self.init.is_some())
            + self.commits.len()
            + self.delivery_intents.len()
            + self.delivery_acks.len()
    }
}

pub(crate) fn preflight_append(
    location: &super::fs::JournalLocation,
    run_id: &WorkflowRunId,
    kind: WorkflowJournalAppendKind,
    first_revision: Option<u64>,
    expected_identity: Option<&super::fs::FileIdentity>,
) -> Result<WorkflowJournalAppendDecision, WorkflowJournalError> {
    let replay = replay(location, run_id, expected_identity)?;
    preflight::validate(location.display_path(), &replay, &kind, first_revision)
}

pub(crate) fn prepare_commit(
    location: &super::fs::JournalLocation,
    run_id: &WorkflowRunId,
    first_revision: Option<u64>,
    expected_identity: Option<&super::fs::FileIdentity>,
) -> Result<std::fs::File, WorkflowJournalError> {
    let path = location.display_path();
    let mut opened = super::fs::open(location, super::fs::OpenMode::AppendExisting)?
        .ok_or_else(|| corrupt(path, 0, "commit requires an initialized journal"))?;
    super::fs::verify_identity(path, expected_identity, &opened.identity)?;
    let replay = read::opened(
        path,
        location.session_id(),
        run_id,
        &mut opened.file,
        opened.identity,
    )?;
    preflight::validate(
        path,
        &replay,
        &WorkflowJournalAppendKind::Commit,
        first_revision,
    )?;
    Ok(opened.file)
}

pub(crate) fn prepare_delivery(
    location: &super::fs::JournalLocation,
    run_id: &WorkflowRunId,
    kind: WorkflowJournalAppendKind,
    expected_identity: Option<&super::fs::FileIdentity>,
) -> Result<Option<std::fs::File>, WorkflowJournalError> {
    let path = location.display_path();
    let mut opened = super::fs::open(location, super::fs::OpenMode::AppendExisting)?
        .ok_or_else(|| corrupt(path, 0, "delivery record requires an initialized journal"))?;
    super::fs::verify_identity(path, expected_identity, &opened.identity)?;
    let replay = read::opened(
        path,
        location.session_id(),
        run_id,
        &mut opened.file,
        opened.identity,
    )?;
    match preflight::validate(path, &replay, &kind, None)? {
        WorkflowJournalAppendDecision::Append => Ok(Some(opened.file)),
        WorkflowJournalAppendDecision::AlreadyPresent => Ok(None),
    }
}

pub(crate) fn replay(
    location: &super::fs::JournalLocation,
    run_id: &WorkflowRunId,
    expected_identity: Option<&super::fs::FileIdentity>,
) -> Result<WorkflowJournalReplay, WorkflowJournalError> {
    let path = location.display_path();
    let Some(mut opened) = super::fs::open(location, super::fs::OpenMode::Read)? else {
        return Ok(Default::default());
    };
    super::fs::verify_identity(path, expected_identity, &opened.identity)?;
    read::opened(
        path,
        location.session_id(),
        run_id,
        &mut opened.file,
        opened.identity,
    )
}

pub(crate) fn repair(
    location: &super::fs::JournalLocation,
    tail: TornTail,
) -> Result<(), WorkflowJournalError> {
    repair::run(location, tail)
}

pub(super) fn corrupt(
    path: &std::path::Path,
    offset: u64,
    detail: impl Into<String>,
) -> WorkflowJournalError {
    WorkflowJournalError::Corruption {
        path: path.to_path_buf(),
        offset,
        detail: detail.into(),
    }
}
