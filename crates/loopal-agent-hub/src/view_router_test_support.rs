use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};

use loopal_ipc::Connection;
use loopal_protocol::{
    AgentEvent, QualifiedAddress, ROOT_AGENT_NAME, WORKFLOW_SPEC_V1, WorkflowAgentNode,
    WorkflowLimits, WorkflowNodeId, WorkflowOutputContract, WorkflowRequestId, WorkflowRunId,
    WorkflowRunSnapshot, WorkflowRunState, WorkflowSpec, WorkflowStartRequest,
    WorkflowTerminalDeliveryId, WorkflowTerminalNotification, WorkflowWorkerProfileRef,
};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

use crate::Hub;
use crate::topology::AgentLifecycle;
use crate::types::{AgentExecutionRef, AgentRuntimeFacts, SpawnAuthority};
use crate::workflow::journal::{
    RecoveredOwner, StartJournalRecord, WorkflowJournalDeliveryAckOutcome,
    WorkflowJournalDeliveryIntentOutcome, WorkflowJournalStorage,
};
use crate::workflow::{
    SystemWorkflowClock, SystemWorkflowIdSource, WorkflowCoordinator, WorkflowCoordinatorError,
    WorkflowCoordinatorHandle, WorkflowOwner,
};

pub(crate) struct TestJournal {
    recoveries: StdMutex<VecDeque<Result<RecoveredOwner, WorkflowCoordinatorError>>>,
}

impl TestJournal {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            recoveries: StdMutex::new(VecDeque::new()),
        })
    }

    pub(crate) fn push_recovery(&self, recovery: RecoveredOwner) {
        self.recoveries.lock().unwrap().push_back(Ok(recovery));
    }
}

impl WorkflowJournalStorage for TestJournal {
    fn recover(&self, _owner: &WorkflowOwner) -> Result<RecoveredOwner, WorkflowCoordinatorError> {
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

    fn append_start(&self, _record: StartJournalRecord) -> Result<(), WorkflowCoordinatorError> {
        Ok(())
    }

    fn append_request(
        &self,
        _owner: &WorkflowOwner,
        _run_id: &WorkflowRunId,
        _request: loopal_protocol::WorkflowRequestRecord,
    ) -> Result<(), WorkflowCoordinatorError> {
        Ok(())
    }

    fn append_commit(
        &self,
        _owner: &WorkflowOwner,
        _run_id: &WorkflowRunId,
        _events: Vec<loopal_protocol::WorkflowEvent>,
        _request: Option<loopal_protocol::WorkflowRequestRecord>,
    ) -> Result<(), WorkflowCoordinatorError> {
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

pub(crate) fn root_hub(session_id: Option<&str>) -> (Arc<Mutex<Hub>>, AgentExecutionRef) {
    let (events, _receiver) = mpsc::channel::<AgentEvent>(64);
    let mut hub = Hub::new(events);
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = Connection::new(transport).into_listening().0;
    let execution = hub
        .registry
        .register_connection_with_parent_execution(ROOT_AGENT_NAME, connection, None, None, None)
        .unwrap();
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.session_id = session_id.map(str::to_owned);
    assert!(hub.registry.set_runtime_facts(&execution, facts));
    hub.registry
        .set_lifecycle(ROOT_AGENT_NAME, AgentLifecycle::Running);
    (Arc::new(Mutex::new(hub)), execution)
}

pub(crate) fn coordinator(
    journal: Arc<TestJournal>,
) -> (WorkflowCoordinatorHandle, JoinHandle<()>) {
    WorkflowCoordinator::spawn_for_test(
        crate::workflow::WorkflowCoordinatorMode::Preview,
        Arc::new(SystemWorkflowClock),
        Arc::new(SystemWorkflowIdSource),
        journal,
    )
}

pub(crate) fn owner(session_id: &str) -> WorkflowOwner {
    WorkflowOwner::new(session_id, QualifiedAddress::local(ROOT_AGENT_NAME))
}

pub(crate) fn request(request_id: &str) -> WorkflowStartRequest {
    WorkflowStartRequest {
        request_id: WorkflowRequestId::new(request_id),
        spec: spec(),
    }
}

pub(crate) fn spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "reconcile workflow snapshot".into(),
        nodes: vec![
            node("source", Vec::new()),
            node("output", vec!["source".into()]),
        ],
        limits: WorkflowLimits {
            max_nodes: 8,
            max_parallel: 2,
            max_attempts: 8,
            run_deadline_ms: 60_000,
            attempt_timeout_ms: 30_000,
            max_output_bytes: 4_096,
        },
        output_node: WorkflowNodeId::from("output"),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}

fn node(id: &str, dependencies: Vec<WorkflowNodeId>) -> WorkflowAgentNode {
    WorkflowAgentNode {
        id: WorkflowNodeId::from(id),
        dependencies,
        task: format!("complete {id}"),
        worker_profile: WorkflowWorkerProfileRef::new("default"),
    }
}

pub(crate) fn recovered_run(id: &str) -> WorkflowRunSnapshot {
    let mut run = WorkflowRunSnapshot::planned(
        WorkflowRunId::new(id),
        QualifiedAddress::local(ROOT_AGENT_NAME),
        spec(),
        10,
    );
    run.state = WorkflowRunState::Running;
    run.revision = 1;
    run.updated_at_unix_ms = 20;
    run
}

pub(crate) async fn shutdown(
    hub: &Arc<Mutex<Hub>>,
    handle: WorkflowCoordinatorHandle,
    task: JoinHandle<()>,
) {
    hub.lock().await.clear_workflow_coordinator();
    handle.shutdown().await.unwrap();
    task.await.unwrap();
}
