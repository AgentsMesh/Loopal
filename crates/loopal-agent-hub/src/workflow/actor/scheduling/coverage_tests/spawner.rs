struct TestSpawner {
    adoption: Mutex<Option<Result<WorkflowPreparedWorker, WorkflowRecoveryAdoptionError>>>,
    abort_status: WorkflowCleanupStatus,
    shutdown_status: WorkflowCleanupStatus,
    panic_abort: AtomicBool,
    shutdowns: AtomicUsize,
    outcomes: Mutex<Vec<oneshot::Sender<WorkflowWorkerOutcome>>>,
}

impl TestSpawner {
    fn confirmed() -> Arc<Self> {
        Arc::new(Self {
            adoption: Mutex::new(None),
            abort_status: WorkflowCleanupStatus::Confirmed,
            shutdown_status: WorkflowCleanupStatus::Confirmed,
            panic_abort: AtomicBool::new(false),
            shutdowns: AtomicUsize::new(0),
            outcomes: Mutex::new(Vec::new()),
        })
    }

    fn timed_out() -> Arc<Self> {
        Arc::new(Self {
            adoption: Mutex::new(None),
            abort_status: WorkflowCleanupStatus::TimedOut,
            shutdown_status: WorkflowCleanupStatus::TimedOut,
            panic_abort: AtomicBool::new(false),
            shutdowns: AtomicUsize::new(0),
            outcomes: Mutex::new(Vec::new()),
        })
    }

    fn panicking_abort() -> Arc<Self> {
        let spawner = Self::confirmed();
        spawner.panic_abort.store(true, Ordering::SeqCst);
        spawner
    }

    fn adopt_error(error: WorkflowRecoveryAdoptionError) -> Arc<Self> {
        let spawner = Self::confirmed();
        *spawner.adoption.lock().unwrap() = Some(Err(error));
        spawner
    }

    fn adopt_worker(self: &Arc<Self>, execution: AgentExecutionRef) {
        let (outcome, receiver) = oneshot::channel();
        self.outcomes.lock().unwrap().push(outcome);
        *self.adoption.lock().unwrap() = Some(Ok(WorkflowPreparedWorker {
            execution,
            outcome: receiver,
        }));
    }
}

#[async_trait::async_trait]
impl WorkflowSpawner for TestSpawner {
    async fn prepare(
        &self,
        _request: WorkflowSpawnRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure> {
        Err(spawn_failure(
            WorkflowFailureClass::Permanent,
            "unused prepare",
        ))
    }

    async fn abort_prepare_and_wait(
        &self,
        _causation: &WorkflowPermissionCausation,
        _timeout: Duration,
    ) -> WorkflowCleanupStatus {
        assert!(
            !self.panic_abort.load(Ordering::SeqCst),
            "injected preparation cleanup panic"
        );
        self.abort_status
    }

    async fn adopt_recovered(
        &self,
        _request: WorkflowRecoveryAdoptionRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowRecoveryAdoptionError> {
        self.adoption
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Err(WorkflowRecoveryAdoptionError::MissingCustody))
    }

    async fn activate(
        &self,
        _execution: &AgentExecutionRef,
    ) -> Result<(), WorkflowActivationFailure> {
        Ok(())
    }

    async fn interrupt(&self, _execution: &AgentExecutionRef) -> WorkflowStopStatus {
        WorkflowStopStatus::Stopped
    }

    async fn shutdown_and_wait(
        &self,
        _execution: &AgentExecutionRef,
        _timeout: Duration,
    ) -> WorkflowCleanupStatus {
        self.shutdowns.fetch_add(1, Ordering::SeqCst);
        self.shutdown_status
    }
}
