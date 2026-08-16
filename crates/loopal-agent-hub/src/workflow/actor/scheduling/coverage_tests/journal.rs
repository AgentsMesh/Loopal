#[derive(Default)]
struct MemoryJournal {
    fail_commits: AtomicBool,
    payloads: Mutex<Vec<WorkflowEventPayload>>,
}

impl MemoryJournal {
    fn failing() -> Arc<Self> {
        Arc::new(Self {
            fail_commits: AtomicBool::new(true),
            payloads: Mutex::new(Vec::new()),
        })
    }

    fn payloads(&self) -> Vec<WorkflowEventPayload> {
        self.payloads.lock().unwrap().clone()
    }
}

impl WorkflowJournalStorage for MemoryJournal {
    fn recover(&self, _owner: &WorkflowOwner) -> Result<RecoveredOwner, WorkflowCoordinatorError> {
        Ok(RecoveredOwner {
            runs: Vec::new(),
            requests: Default::default(),
            delivery_intents: Vec::new(),
            acked_deliveries: Default::default(),
        })
    }

    fn append_start(&self, _record: StartJournalRecord) -> Result<(), WorkflowCoordinatorError> {
        Ok(())
    }

    fn append_request(
        &self,
        _owner: &WorkflowOwner,
        _run_id: &WorkflowRunId,
        _request: WorkflowRequestRecord,
    ) -> Result<(), WorkflowCoordinatorError> {
        Ok(())
    }

    fn append_commit(
        &self,
        _owner: &WorkflowOwner,
        _run_id: &WorkflowRunId,
        events: Vec<WorkflowEvent>,
        _request: Option<WorkflowRequestRecord>,
    ) -> Result<(), WorkflowCoordinatorError> {
        if self.fail_commits.load(Ordering::SeqCst) {
            return Err(WorkflowCoordinatorError::JournalUnavailable);
        }
        self.payloads
            .lock()
            .unwrap()
            .extend(events.into_iter().map(|event| event.payload));
        Ok(())
    }

    fn append_delivery_ack(
        &self,
        _owner: &WorkflowOwner,
        _delivery_id: &WorkflowTerminalDeliveryId,
    ) -> Result<WorkflowJournalDeliveryAckOutcome, WorkflowCoordinatorError> {
        Ok(WorkflowJournalDeliveryAckOutcome::Appended)
    }

    fn append_delivery_intent(
        &self,
        _owner: &WorkflowOwner,
        notification: WorkflowTerminalNotification,
    ) -> Result<WorkflowJournalDeliveryIntentOutcome, WorkflowCoordinatorError> {
        Ok(WorkflowJournalDeliveryIntentOutcome::Appended(notification))
    }
}

struct TestClock(AtomicU64);

impl TestClock {
    fn new(now: u64) -> Self {
        Self(AtomicU64::new(now))
    }
}

impl WorkflowClock for TestClock {
    fn now_unix_ms(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
}
