mod append;
mod delivery;
mod error;
pub(crate) mod fs;
mod io;
mod record;
mod replay;
mod strict;
mod validate;

use loopal_protocol::{
    WorkflowEvent, WorkflowRequestRecord, WorkflowRunId, WorkflowRunSnapshot,
    WorkflowTerminalDeliveryId, WorkflowTerminalNotification,
};

pub use error::{WorkflowJournalError, WorkflowJournalLimit};
pub use record::{
    WORKFLOW_JOURNAL_VERSION, WorkflowJournalCommit, WorkflowJournalEntry, WorkflowJournalInit,
};
pub use replay::{TornTail, WorkflowJournalReplay};

pub const MAX_WORKFLOW_JOURNAL_LINE_BYTES: usize = 16 * 1_024 * 1_024;
pub const MAX_WORKFLOW_JOURNAL_TOTAL_BYTES: u64 = 256 * 1_024 * 1_024;
pub const MAX_WORKFLOW_JOURNAL_ENTRIES: usize = 16_384;
pub const MAX_WORKFLOW_JOURNALS_PER_SESSION: usize = 64;
pub const MAX_WORKFLOW_SESSION_JOURNAL_BYTES: u64 = 512 * 1_024 * 1_024;
pub const MAX_WORKFLOW_EVENTS_PER_COMMIT: usize = 512;
pub const MAX_WORKFLOW_REQUEST_RECORD_BYTES: usize = 2 * 1_024 * 1_024;

#[derive(Clone, Debug)]
pub struct WorkflowJournal {
    location: fs::JournalLocation,
    run_id: WorkflowRunId,
    expected_identity: Option<fs::FileIdentity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowJournalAppendKind {
    Init,
    Commit,
    DeliveryIntent(WorkflowTerminalNotification),
    DeliveryAck(WorkflowTerminalDeliveryId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowJournalAppendDecision {
    Append,
    AlreadyPresent,
}

impl WorkflowJournal {
    pub fn from_session_store(
        sessions: &crate::SessionStore,
        session_id: &str,
        run_id: WorkflowRunId,
    ) -> Result<Self, WorkflowJournalError> {
        if !run_id.is_valid() {
            return Err(WorkflowJournalError::InvalidRunId(run_id.to_string()));
        }
        let path = sessions.workflow_journal_path(session_id, run_id.as_str())?;
        let location = fs::JournalLocation::new(
            sessions.base_dir(),
            session_id,
            run_id.as_str(),
            path.clone(),
        );
        Ok(Self {
            location,
            run_id,
            expected_identity: None,
        })
    }

    pub(crate) fn from_discovered(
        sessions: &crate::SessionStore,
        session_id: &str,
        run_id: WorkflowRunId,
        identity: fs::FileIdentity,
    ) -> Result<Self, WorkflowJournalError> {
        let mut journal = Self::from_session_store(sessions, session_id, run_id)?;
        journal.expected_identity = Some(identity);
        Ok(journal)
    }

    pub fn run_id(&self) -> &WorkflowRunId {
        &self.run_id
    }

    pub fn preflight_append(
        &self,
        kind: WorkflowJournalAppendKind,
    ) -> Result<WorkflowJournalAppendDecision, WorkflowJournalError> {
        match &kind {
            WorkflowJournalAppendKind::DeliveryIntent(notification) => validate::entry(
                self.location.session_id(),
                &self.run_id,
                &WorkflowJournalEntry::delivery_intent(notification.clone()),
            )?,
            WorkflowJournalAppendKind::DeliveryAck(delivery_id) => validate::entry(
                self.location.session_id(),
                &self.run_id,
                &WorkflowJournalEntry::delivery_ack(delivery_id.clone()),
            )?,
            WorkflowJournalAppendKind::Init | WorkflowJournalAppendKind::Commit => {}
        }
        replay::preflight_append(
            &self.location,
            &self.run_id,
            kind,
            None,
            self.expected_identity.as_ref(),
        )
    }

    pub fn append_init(&self, snapshot: WorkflowRunSnapshot) -> Result<(), WorkflowJournalError> {
        self.append_init_with_request(snapshot, None)
    }

    pub fn append_init_with_request(
        &self,
        snapshot: WorkflowRunSnapshot,
        request: Option<WorkflowRequestRecord>,
    ) -> Result<(), WorkflowJournalError> {
        self.append_init_with_events(snapshot, Vec::new(), request)
    }

    pub fn append_init_with_events(
        &self,
        snapshot: WorkflowRunSnapshot,
        events: Vec<WorkflowEvent>,
        request: Option<WorkflowRequestRecord>,
    ) -> Result<(), WorkflowJournalError> {
        let line = append::prepare(
            self.location.session_id(),
            &self.run_id,
            &WorkflowJournalEntry::init(snapshot, events, request),
        )?;
        append::append_init(&self.location, &line)
    }

    pub fn append_commit(
        &self,
        events: Vec<WorkflowEvent>,
        request: Option<WorkflowRequestRecord>,
    ) -> Result<(), WorkflowJournalError> {
        let first_revision = events.first().map(|event| event.revision);
        let entry = WorkflowJournalEntry::commit(self.run_id.clone(), events, request);
        let line = append::prepare(self.location.session_id(), &self.run_id, &entry)?;
        let file = replay::prepare_commit(
            &self.location,
            &self.run_id,
            first_revision,
            self.expected_identity.as_ref(),
        )?;
        append::append_commit(self.location.display_path(), &line, file)
    }

    pub fn replay(&self) -> Result<WorkflowJournalReplay, WorkflowJournalError> {
        replay::replay(
            &self.location,
            &self.run_id,
            self.expected_identity.as_ref(),
        )
    }

    pub fn repair_torn_tail(&self, tail: TornTail) -> Result<(), WorkflowJournalError> {
        replay::repair(&self.location, tail)
    }
}
