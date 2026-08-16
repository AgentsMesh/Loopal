use loopal_protocol::{WorkflowTerminalDeliveryId, WorkflowTerminalNotification};

use super::{
    WorkflowJournal, WorkflowJournalAppendDecision, WorkflowJournalAppendKind,
    WorkflowJournalEntry, WorkflowJournalError, append, replay,
};

impl WorkflowJournal {
    pub fn append_delivery_intent(
        &self,
        notification: WorkflowTerminalNotification,
    ) -> Result<WorkflowJournalAppendDecision, WorkflowJournalError> {
        let entry = WorkflowJournalEntry::delivery_intent(notification.clone());
        self.append_delivery_entry(
            entry,
            WorkflowJournalAppendKind::DeliveryIntent(notification),
        )
    }

    pub fn append_delivery_ack(
        &self,
        delivery_id: WorkflowTerminalDeliveryId,
    ) -> Result<WorkflowJournalAppendDecision, WorkflowJournalError> {
        let entry = WorkflowJournalEntry::delivery_ack(delivery_id.clone());
        self.append_delivery_entry(entry, WorkflowJournalAppendKind::DeliveryAck(delivery_id))
    }

    fn append_delivery_entry(
        &self,
        entry: WorkflowJournalEntry,
        kind: WorkflowJournalAppendKind,
    ) -> Result<WorkflowJournalAppendDecision, WorkflowJournalError> {
        let line = append::prepare(self.location.session_id(), &self.run_id, &entry)?;
        let Some(file) = replay::prepare_delivery(
            &self.location,
            &self.run_id,
            kind,
            self.expected_identity.as_ref(),
        )?
        else {
            return Ok(WorkflowJournalAppendDecision::AlreadyPresent);
        };
        append::append_commit(self.location.display_path(), &line, file)?;
        Ok(WorkflowJournalAppendDecision::Append)
    }
}
