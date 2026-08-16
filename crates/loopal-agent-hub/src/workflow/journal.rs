use std::sync::Arc;

#[path = "journal/session.rs"]
mod session;

use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{
    WorkflowEvent, WorkflowRequestRecord, WorkflowRunId, WorkflowRunSnapshot,
    WorkflowTerminalDeliveryId, WorkflowTerminalNotification,
};
use loopal_storage::SessionStore;

pub(crate) use super::recovery::RecoveredOwner;
use super::{WorkflowCoordinatorError, WorkflowOwner};

#[derive(Clone)]
pub(crate) struct StartJournalRecord {
    pub(crate) owner: WorkflowOwner,
    pub(crate) planned: WorkflowRunSnapshot,
    pub(crate) event: WorkflowEvent,
    pub(crate) request: WorkflowRequestRecord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkflowJournalDeliveryAckOutcome {
    Appended,
    AlreadyPresent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WorkflowJournalDeliveryIntentOutcome {
    Appended(WorkflowTerminalNotification),
    AlreadyPresent(WorkflowTerminalNotification),
}

pub(crate) trait WorkflowJournalStorage: Send + Sync + 'static {
    fn recover(&self, owner: &WorkflowOwner) -> Result<RecoveredOwner, WorkflowCoordinatorError>;
    fn append_start(&self, record: StartJournalRecord) -> Result<(), WorkflowCoordinatorError>;
    fn append_request(
        &self,
        owner: &WorkflowOwner,
        run_id: &WorkflowRunId,
        request: WorkflowRequestRecord,
    ) -> Result<(), WorkflowCoordinatorError>;
    fn append_commit(
        &self,
        owner: &WorkflowOwner,
        run_id: &WorkflowRunId,
        events: Vec<WorkflowEvent>,
        request: Option<WorkflowRequestRecord>,
    ) -> Result<(), WorkflowCoordinatorError>;
    fn append_delivery_ack(
        &self,
        owner: &WorkflowOwner,
        delivery_id: &WorkflowTerminalDeliveryId,
    ) -> Result<WorkflowJournalDeliveryAckOutcome, WorkflowCoordinatorError>;
    fn append_delivery_intent(
        &self,
        owner: &WorkflowOwner,
        notification: WorkflowTerminalNotification,
    ) -> Result<WorkflowJournalDeliveryIntentOutcome, WorkflowCoordinatorError>;
}

pub(super) struct UnavailableWorkflowJournals;

impl WorkflowJournalStorage for UnavailableWorkflowJournals {
    fn recover(&self, _owner: &WorkflowOwner) -> Result<RecoveredOwner, WorkflowCoordinatorError> {
        Err(WorkflowCoordinatorError::JournalUnavailable)
    }

    fn append_start(&self, _record: StartJournalRecord) -> Result<(), WorkflowCoordinatorError> {
        Err(WorkflowCoordinatorError::JournalUnavailable)
    }

    fn append_request(
        &self,
        _owner: &WorkflowOwner,
        _run_id: &WorkflowRunId,
        _request: WorkflowRequestRecord,
    ) -> Result<(), WorkflowCoordinatorError> {
        Err(WorkflowCoordinatorError::JournalUnavailable)
    }

    fn append_commit(
        &self,
        _owner: &WorkflowOwner,
        _run_id: &WorkflowRunId,
        _events: Vec<WorkflowEvent>,
        _request: Option<WorkflowRequestRecord>,
    ) -> Result<(), WorkflowCoordinatorError> {
        Err(WorkflowCoordinatorError::JournalUnavailable)
    }

    fn append_delivery_ack(
        &self,
        _owner: &WorkflowOwner,
        _delivery_id: &WorkflowTerminalDeliveryId,
    ) -> Result<WorkflowJournalDeliveryAckOutcome, WorkflowCoordinatorError> {
        Err(WorkflowCoordinatorError::JournalUnavailable)
    }

    fn append_delivery_intent(
        &self,
        _owner: &WorkflowOwner,
        _notification: WorkflowTerminalNotification,
    ) -> Result<WorkflowJournalDeliveryIntentOutcome, WorkflowCoordinatorError> {
        Err(WorkflowCoordinatorError::JournalUnavailable)
    }
}

pub(super) struct SessionWorkflowJournals {
    sessions: Arc<SessionStore>,
    redaction_seed: FinalSinkRedactionSeed,
}

impl SessionWorkflowJournals {
    pub(super) fn new(sessions: Arc<SessionStore>) -> Self {
        Self::new_with_redaction_seed(sessions, FinalSinkRedactionSeed::new())
    }

    pub(super) fn new_with_redaction_seed(
        sessions: Arc<SessionStore>,
        redaction_seed: FinalSinkRedactionSeed,
    ) -> Self {
        Self {
            sessions,
            redaction_seed,
        }
    }
}
