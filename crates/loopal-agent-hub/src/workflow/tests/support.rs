use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use loopal_protocol::{
    QualifiedAddress, WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowAttemptCapability,
    WorkflowAttemptId, WorkflowGetRequest, WorkflowLimits, WorkflowOutputContract,
    WorkflowRequestId, WorkflowRunId, WorkflowSpec, WorkflowStartRequest, WorkflowWorkerProfileRef,
};
use tokio::task::JoinHandle;

use super::super::journal::WorkflowJournalStorage;
use super::super::{
    WorkflowClock, WorkflowCoordinator, WorkflowCoordinatorHandle, WorkflowCoordinatorMode,
    WorkflowIdSource, WorkflowOwner,
};
use super::journal_support::TestJournal;

pub(super) struct TestClock {
    values: Mutex<VecDeque<u64>>,
    calls: AtomicUsize,
}

impl TestClock {
    pub(super) fn new(values: impl IntoIterator<Item = u64>) -> Self {
        Self {
            values: Mutex::new(values.into_iter().collect()),
            calls: AtomicUsize::new(0),
        }
    }

    pub(super) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl WorkflowClock for TestClock {
    fn now_unix_ms(&self) -> u64 {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.values.lock().unwrap().pop_front().unwrap()
    }
}

pub(super) struct TestIds {
    runs: Mutex<VecDeque<WorkflowRunId>>,
    attempts: Mutex<VecDeque<WorkflowAttemptId>>,
    capabilities: Mutex<VecDeque<WorkflowAttemptCapability>>,
    run_calls: AtomicUsize,
    attempt_calls: AtomicUsize,
}

impl TestIds {
    pub(super) fn new(values: impl IntoIterator<Item = WorkflowRunId>) -> Self {
        Self::with_attempts(values, [])
    }

    pub(super) fn with_attempts(
        runs: impl IntoIterator<Item = WorkflowRunId>,
        attempts: impl IntoIterator<Item = WorkflowAttemptId>,
    ) -> Self {
        Self {
            runs: Mutex::new(runs.into_iter().collect()),
            attempts: Mutex::new(attempts.into_iter().collect()),
            capabilities: Mutex::new(VecDeque::new()),
            run_calls: AtomicUsize::new(0),
            attempt_calls: AtomicUsize::new(0),
        }
    }

    pub(super) fn calls(&self) -> usize {
        self.run_calls.load(Ordering::SeqCst)
    }

    pub(super) fn attempt_calls(&self) -> usize {
        self.attempt_calls.load(Ordering::SeqCst)
    }
}

impl WorkflowIdSource for TestIds {
    fn next_run_id(&self) -> WorkflowRunId {
        self.run_calls.fetch_add(1, Ordering::SeqCst);
        self.runs.lock().unwrap().pop_front().unwrap()
    }

    fn next_attempt_id(&self) -> WorkflowAttemptId {
        self.attempt_calls.fetch_add(1, Ordering::SeqCst);
        self.attempts.lock().unwrap().pop_front().unwrap()
    }

    fn next_attempt_capability(&self) -> WorkflowAttemptCapability {
        self.capabilities
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| WorkflowAttemptCapability::parse("11".repeat(32)).unwrap())
    }
}

pub(super) fn coordinator(
    mode: WorkflowCoordinatorMode,
    times: impl IntoIterator<Item = u64>,
    ids: impl IntoIterator<Item = WorkflowRunId>,
) -> (
    WorkflowCoordinatorHandle,
    JoinHandle<()>,
    Arc<TestClock>,
    Arc<TestIds>,
) {
    let (handle, task, clock, ids, _) = coordinator_with_journal(mode, times, ids);
    (handle, task, clock, ids)
}

pub(super) fn coordinator_with_storage(
    mode: WorkflowCoordinatorMode,
    times: impl IntoIterator<Item = u64>,
    ids: impl IntoIterator<Item = WorkflowRunId>,
    journal: Arc<dyn WorkflowJournalStorage>,
) -> (
    WorkflowCoordinatorHandle,
    JoinHandle<()>,
    Arc<TestClock>,
    Arc<TestIds>,
) {
    let clock = Arc::new(TestClock::new(times));
    let ids = Arc::new(TestIds::new(ids));
    let (handle, task) =
        WorkflowCoordinator::spawn_for_test(mode, clock.clone(), ids.clone(), journal);
    (handle, task, clock, ids)
}

pub(super) fn coordinator_with_journal(
    mode: WorkflowCoordinatorMode,
    times: impl IntoIterator<Item = u64>,
    ids: impl IntoIterator<Item = WorkflowRunId>,
) -> (
    WorkflowCoordinatorHandle,
    JoinHandle<()>,
    Arc<TestClock>,
    Arc<TestIds>,
    Arc<TestJournal>,
) {
    let clock = Arc::new(TestClock::new(times));
    let ids = Arc::new(TestIds::new(ids));
    let journal = Arc::new(TestJournal::new());
    let (handle, task) =
        WorkflowCoordinator::spawn_for_test(mode, clock.clone(), ids.clone(), journal.clone());
    (handle, task, clock, ids, journal)
}

pub(super) fn owner(session: &str, root: &str) -> WorkflowOwner {
    WorkflowOwner::new(session, QualifiedAddress::local(root))
}

pub(super) fn request(request_id: &str) -> WorkflowStartRequest {
    WorkflowStartRequest {
        request_id: WorkflowRequestId::new(request_id),
        spec: spec(),
    }
}

pub(super) fn get_request(request_id: &str, run_id: WorkflowRunId) -> WorkflowGetRequest {
    WorkflowGetRequest {
        request_id: WorkflowRequestId::new(request_id),
        run_id,
    }
}

pub(super) fn spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "complete the workflow".into(),
        nodes: vec![node("source", &[]), node("output", &["source"])],
        limits: WorkflowLimits {
            max_nodes: 8,
            max_parallel: 2,
            max_attempts: 8,
            run_deadline_ms: 60_000,
            attempt_timeout_ms: 30_000,
            max_output_bytes: 4_096,
        },
        output_node: "output".into(),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}

fn node(id: &str, dependencies: &[&str]) -> WorkflowAgentNode {
    WorkflowAgentNode {
        id: id.into(),
        dependencies: dependencies.iter().copied().map(Into::into).collect(),
        task: format!("complete {id}"),
        worker_profile: WorkflowWorkerProfileRef::new("default"),
    }
}
