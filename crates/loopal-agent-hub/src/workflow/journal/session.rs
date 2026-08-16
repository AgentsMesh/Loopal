use loopal_output_guard::OutputGuard;
use loopal_protocol::{
    WorkflowEvent, WorkflowRequestRecord, WorkflowRunId, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification,
};
use loopal_storage::{WorkflowJournal, WorkflowJournalAppendDecision};

use super::{
    SessionWorkflowJournals, StartJournalRecord, WorkflowJournalDeliveryAckOutcome,
    WorkflowJournalDeliveryIntentOutcome, WorkflowJournalStorage,
};
use crate::workflow::recovery::{RecoveredOwner, recover_owner};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

impl WorkflowJournalStorage for SessionWorkflowJournals {
    fn recover(&self, owner: &WorkflowOwner) -> Result<RecoveredOwner, WorkflowCoordinatorError> {
        let journals = self.sessions.list_workflow_journals(&owner.session_id)?;
        let mut replays = Vec::with_capacity(journals.len());
        for journal in journals {
            let mut replay = journal.replay()?;
            if let Some(tail) = replay.torn_tail.take() {
                journal.repair_torn_tail(tail)?;
                replay = journal.replay()?;
            }
            replays.push(replay);
        }
        recover_owner(owner, replays)
    }

    fn append_start(&self, record: StartJournalRecord) -> Result<(), WorkflowCoordinatorError> {
        let planned = self.guard_value(record.planned)?;
        let event = self.guard_value(record.event)?;
        let request = self.guard_value(record.request)?;
        let journal = self.journal(&record.owner, &planned.id)?;
        journal.append_init_with_events(planned, vec![event], Some(request))?;
        Ok(())
    }

    fn append_request(
        &self,
        owner: &WorkflowOwner,
        run_id: &WorkflowRunId,
        request: WorkflowRequestRecord,
    ) -> Result<(), WorkflowCoordinatorError> {
        self.journal(owner, run_id)?
            .append_commit(Vec::new(), Some(self.guard_value(request)?))?;
        Ok(())
    }

    fn append_commit(
        &self,
        owner: &WorkflowOwner,
        run_id: &WorkflowRunId,
        events: Vec<WorkflowEvent>,
        request: Option<WorkflowRequestRecord>,
    ) -> Result<(), WorkflowCoordinatorError> {
        let events = self.guard_value(events)?;
        let request = request.map(|record| self.guard_value(record)).transpose()?;
        self.journal(owner, run_id)?
            .append_commit(events, request)?;
        Ok(())
    }

    fn append_delivery_ack(
        &self,
        owner: &WorkflowOwner,
        delivery_id: &WorkflowTerminalDeliveryId,
    ) -> Result<WorkflowJournalDeliveryAckOutcome, WorkflowCoordinatorError> {
        if delivery_id.session_id != owner.session_id {
            return Err(WorkflowCoordinatorError::RecoveryInvalid);
        }
        match self
            .journal(owner, &delivery_id.run_id)?
            .append_delivery_ack(delivery_id.clone())?
        {
            WorkflowJournalAppendDecision::Append => {
                Ok(WorkflowJournalDeliveryAckOutcome::Appended)
            }
            WorkflowJournalAppendDecision::AlreadyPresent => {
                Ok(WorkflowJournalDeliveryAckOutcome::AlreadyPresent)
            }
        }
    }

    fn append_delivery_intent(
        &self,
        owner: &WorkflowOwner,
        notification: WorkflowTerminalNotification,
    ) -> Result<WorkflowJournalDeliveryIntentOutcome, WorkflowCoordinatorError> {
        if notification.delivery_id.session_id != owner.session_id {
            return Err(WorkflowCoordinatorError::RecoveryInvalid);
        }
        let notification = self.guard_value(notification)?;
        notification
            .validate()
            .map_err(|_| WorkflowCoordinatorError::RecoveryInvalid)?;
        match self
            .journal(owner, &notification.delivery_id.run_id)?
            .append_delivery_intent(notification.clone())?
        {
            WorkflowJournalAppendDecision::Append => {
                Ok(WorkflowJournalDeliveryIntentOutcome::Appended(notification))
            }
            WorkflowJournalAppendDecision::AlreadyPresent => Ok(
                WorkflowJournalDeliveryIntentOutcome::AlreadyPresent(notification),
            ),
        }
    }
}

impl SessionWorkflowJournals {
    fn guard_value<T>(&self, value: T) -> Result<T, WorkflowCoordinatorError>
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let snapshot = self
            .redaction_seed
            .snapshot()
            .map_err(|_| encoding_failure())?;
        let guard = OutputGuard::new(&snapshot).map_err(|_| encoding_failure())?;
        let value = serde_json::to_value(value).map_err(|_| encoding_failure())?;
        let guarded = guard
            .guard_json(&value, loopal_storage::MAX_WORKFLOW_JOURNAL_LINE_BYTES)
            .map_err(|_| encoding_failure())?
            .into_inner()
            .into_value();
        serde_json::from_value(guarded).map_err(|_| encoding_failure())
    }

    fn journal(
        &self,
        owner: &WorkflowOwner,
        run_id: &WorkflowRunId,
    ) -> Result<WorkflowJournal, WorkflowCoordinatorError> {
        WorkflowJournal::from_session_store(
            self.sessions.as_ref(),
            &owner.session_id,
            run_id.clone(),
        )
        .map_err(Into::into)
    }
}

fn encoding_failure() -> WorkflowCoordinatorError {
    WorkflowCoordinatorError::Encoding("output guard rejected workflow journal data".into())
}
