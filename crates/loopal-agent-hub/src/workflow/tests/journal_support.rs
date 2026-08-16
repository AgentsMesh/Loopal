use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use loopal_protocol::{
    WorkflowEvent, WorkflowRequestRecord, WorkflowRunId, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification,
};
use tokio::sync::Notify;

use super::super::journal::StartJournalRecord;
use super::super::recovery::RecoveredOwner;
use super::super::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) struct TestJournal {
    recoveries: Mutex<VecDeque<Result<RecoveredOwner, WorkflowCoordinatorError>>>,
    starts: Mutex<Vec<StartJournalRecord>>,
    requests: Mutex<Vec<(WorkflowOwner, WorkflowRunId, WorkflowRequestRecord)>>,
    events: Mutex<Vec<(WorkflowOwner, WorkflowRunId, Vec<WorkflowEvent>)>>,
    pub(super) delivery_acks: Mutex<Vec<(WorkflowOwner, WorkflowTerminalDeliveryId)>>,
    delivery_intents: Mutex<Vec<(WorkflowOwner, WorkflowTerminalNotification)>>,
    pub(super) append_errors: Mutex<VecDeque<WorkflowCoordinatorError>>,
    pub(super) append_panics: Mutex<usize>,
    recovery_panics: Mutex<usize>,
    event_appended: Notify,
    pub(super) delivery_ack_attempted: Notify,
    pub(super) delivery_ack_appended: Notify,
    pub(super) delivery_ack_attempts: AtomicUsize,
}

impl TestJournal {
    pub(super) fn new() -> Self {
        Self {
            recoveries: Mutex::new(VecDeque::new()),
            starts: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
            events: Mutex::new(Vec::new()),
            delivery_acks: Mutex::new(Vec::new()),
            delivery_intents: Mutex::new(Vec::new()),
            append_errors: Mutex::new(VecDeque::new()),
            append_panics: Mutex::new(0),
            recovery_panics: Mutex::new(0),
            event_appended: Notify::new(),
            delivery_ack_attempted: Notify::new(),
            delivery_ack_appended: Notify::new(),
            delivery_ack_attempts: AtomicUsize::new(0),
        }
    }

    pub(super) fn push_recovery(&self, recovery: Result<RecoveredOwner, WorkflowCoordinatorError>) {
        self.recoveries.lock().unwrap().push_back(recovery);
    }

    pub(super) fn push_recovery_panic(&self) {
        *self.recovery_panics.lock().unwrap() += 1;
    }

    pub(super) fn push_append_error(&self, error: WorkflowCoordinatorError) {
        self.append_errors.lock().unwrap().push_back(error);
    }

    pub(super) fn push_append_panic(&self) {
        *self.append_panics.lock().unwrap() += 1;
    }

    pub(super) fn starts(&self) -> Vec<StartJournalRecord> {
        self.starts.lock().unwrap().clone()
    }

    pub(super) fn requests(&self) -> Vec<(WorkflowOwner, WorkflowRunId, WorkflowRequestRecord)> {
        self.requests.lock().unwrap().clone()
    }

    pub(super) fn events(&self) -> Vec<(WorkflowOwner, WorkflowRunId, Vec<WorkflowEvent>)> {
        self.events.lock().unwrap().clone()
    }

    pub(super) fn delivery_acks(&self) -> Vec<(WorkflowOwner, WorkflowTerminalDeliveryId)> {
        self.delivery_acks.lock().unwrap().clone()
    }

    pub(super) async fn wait_for_event_batches(&self, expected: usize) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let notified = self.event_appended.notified();
                if self.events.lock().unwrap().len() >= expected {
                    return;
                }
                notified.await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {expected} workflow event batches"));
    }

    pub(super) async fn wait_for_delivery_ack_attempt(&self) {
        journal_delivery_ack_support::wait_for_attempt(self).await;
    }

    pub(super) async fn wait_for_delivery_acks(&self, expected: usize) {
        journal_delivery_ack_support::wait_for_acks(self, expected).await;
    }
}

impl TestJournal {
    pub(super) fn recover_next(&self) -> Result<RecoveredOwner, WorkflowCoordinatorError> {
        if journal_delivery_ack_support::consume_panic(&self.recovery_panics) {
            panic!("injected recovery panic");
        }
        self.recoveries
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                Ok(RecoveredOwner {
                    runs: Vec::new(),
                    requests: Default::default(),
                    delivery_intents: Vec::new(),
                    acked_deliveries: Default::default(),
                })
            })
    }
}

impl TestJournal {
    pub(super) fn append_start(
        &self,
        record: StartJournalRecord,
    ) -> Result<(), WorkflowCoordinatorError> {
        if journal_delivery_ack_support::consume_panic(&self.append_panics) {
            panic!("injected append panic");
        }
        if let Some(error) = self.append_errors.lock().unwrap().pop_front() {
            return Err(error);
        }
        self.starts.lock().unwrap().push(record);
        Ok(())
    }

    pub(super) fn append_delivery_ack(
        &self,
        owner: &WorkflowOwner,
        delivery_id: &WorkflowTerminalDeliveryId,
    ) -> Result<super::super::journal::WorkflowJournalDeliveryAckOutcome, WorkflowCoordinatorError>
    {
        journal_delivery_ack_support::append(self, owner, delivery_id)
    }
}

#[path = "journal_delivery_ack_support.rs"]
mod journal_delivery_ack_support;
mod storage;
