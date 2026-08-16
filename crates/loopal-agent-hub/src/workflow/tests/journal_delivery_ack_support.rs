use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Duration;

use loopal_protocol::WorkflowTerminalDeliveryId;

use super::super::super::journal::WorkflowJournalDeliveryAckOutcome;
use super::super::super::{WorkflowCoordinatorError, WorkflowOwner};
use super::TestJournal;

pub(super) fn append(
    journal: &TestJournal,
    owner: &WorkflowOwner,
    delivery_id: &WorkflowTerminalDeliveryId,
) -> Result<WorkflowJournalDeliveryAckOutcome, WorkflowCoordinatorError> {
    journal.delivery_ack_attempts.fetch_add(1, Ordering::SeqCst);
    journal.delivery_ack_attempted.notify_waiters();
    journal.before_append()?;
    let mut acks = journal.delivery_acks.lock().unwrap();
    if acks
        .iter()
        .any(|(existing_owner, existing_id)| existing_owner == owner && existing_id == delivery_id)
    {
        return Ok(WorkflowJournalDeliveryAckOutcome::AlreadyPresent);
    }
    acks.push((owner.clone(), delivery_id.clone()));
    journal.delivery_ack_appended.notify_waiters();
    Ok(WorkflowJournalDeliveryAckOutcome::Appended)
}

pub(super) async fn wait_for_attempt(journal: &TestJournal) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let notified = journal.delivery_ack_attempted.notified();
            if journal.delivery_ack_attempts.load(Ordering::SeqCst) > 0 {
                return;
            }
            notified.await;
        }
    })
    .await
    .expect("timed out waiting for workflow delivery ACK attempt");
}

pub(super) async fn wait_for_acks(journal: &TestJournal, expected: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let notified = journal.delivery_ack_appended.notified();
            if journal.delivery_acks.lock().unwrap().len() >= expected {
                return;
            }
            notified.await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {expected} workflow delivery ACKs"));
}

pub(super) fn consume_panic(panics: &Mutex<usize>) -> bool {
    let mut panics = panics.lock().unwrap();
    if *panics == 0 {
        false
    } else {
        *panics -= 1;
        true
    }
}

impl TestJournal {
    pub(super) fn before_append(&self) -> Result<(), WorkflowCoordinatorError> {
        if consume_panic(&self.append_panics) {
            panic!("injected append panic");
        }
        match self.append_errors.lock().unwrap().pop_front() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
