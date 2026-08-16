use crate::workflow::journal::{
    StartJournalRecord, WorkflowJournalDeliveryAckOutcome, WorkflowJournalDeliveryIntentOutcome,
    WorkflowJournalStorage,
};
use crate::workflow::recovery::RecoveredOwner;
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};
use loopal_protocol::{
    WorkflowEvent, WorkflowRequestRecord, WorkflowRunId, WorkflowTerminalDeliveryId,
};

use super::TestJournal;

impl WorkflowJournalStorage for TestJournal {
    fn recover(&self, _owner: &WorkflowOwner) -> Result<RecoveredOwner, WorkflowCoordinatorError> {
        self.recover_next()
    }

    fn append_start(&self, record: StartJournalRecord) -> Result<(), WorkflowCoordinatorError> {
        TestJournal::append_start(self, record)
    }

    fn append_request(
        &self,
        owner: &WorkflowOwner,
        run_id: &WorkflowRunId,
        request: WorkflowRequestRecord,
    ) -> Result<(), WorkflowCoordinatorError> {
        self.before_append()?;
        self.requests
            .lock()
            .unwrap()
            .push((owner.clone(), run_id.clone(), request));
        Ok(())
    }

    fn append_commit(
        &self,
        owner: &WorkflowOwner,
        run_id: &WorkflowRunId,
        events: Vec<WorkflowEvent>,
        request: Option<WorkflowRequestRecord>,
    ) -> Result<(), WorkflowCoordinatorError> {
        self.before_append()?;
        if let Some(request) = request {
            self.requests
                .lock()
                .unwrap()
                .push((owner.clone(), run_id.clone(), request));
        }
        self.events
            .lock()
            .unwrap()
            .push((owner.clone(), run_id.clone(), events));
        self.event_appended.notify_waiters();
        Ok(())
    }

    fn append_delivery_ack(
        &self,
        owner: &WorkflowOwner,
        delivery_id: &WorkflowTerminalDeliveryId,
    ) -> Result<WorkflowJournalDeliveryAckOutcome, WorkflowCoordinatorError> {
        TestJournal::append_delivery_ack(self, owner, delivery_id)
    }

    fn append_delivery_intent(
        &self,
        owner: &WorkflowOwner,
        notification: loopal_protocol::WorkflowTerminalNotification,
    ) -> Result<WorkflowJournalDeliveryIntentOutcome, WorkflowCoordinatorError> {
        self.before_append()?;
        let mut intents = self.delivery_intents.lock().unwrap();
        if let Some((_, existing)) = intents.iter().find(|(current_owner, current)| {
            current_owner == owner && current.delivery_id == notification.delivery_id
        }) {
            if existing != &notification {
                return Err(WorkflowCoordinatorError::RecoveryInvalid);
            }
            return Ok(WorkflowJournalDeliveryIntentOutcome::AlreadyPresent(
                existing.clone(),
            ));
        }
        intents.push((owner.clone(), notification.clone()));
        Ok(WorkflowJournalDeliveryIntentOutcome::Appended(notification))
    }
}
