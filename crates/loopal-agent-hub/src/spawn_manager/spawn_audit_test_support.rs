use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex as StdMutex};

use loopal_ipc::Connection;
use loopal_protocol::{
    QualifiedAddress, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId,
};
use loopal_vault_api::{AuditError, AuditMetadata, AuditResult, AuditSink, ProtectedOp, VaultOp};
use tokio::sync::{Mutex, Notify, mpsc};

use super::{PreparedSpawn, SpawnRequestLease};
use crate::Hub;
use crate::types::{AgentExecutionRef, AgentRuntimeFacts, SpawnAuthority};

#[derive(Clone, Debug)]
pub(crate) struct Record {
    pub(crate) op: ProtectedOp,
    pub(crate) subject: String,
    pub(crate) session_id: Option<String>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) agent_name: Option<String>,
    pub(crate) depth: Option<u32>,
    pub(crate) generation: Option<u64>,
    pub(crate) workflow_run_id: Option<String>,
    pub(crate) workflow_node_id: Option<String>,
    pub(crate) workflow_attempt_id: Option<String>,
    pub(crate) workflow_phase: Option<String>,
    pub(crate) spawn_target: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) permission_mode: Option<String>,
    pub(crate) decision_mode: Option<String>,
    pub(crate) sandbox_policy: Option<String>,
}

pub(crate) struct Gate {
    started: Notify,
    released: StdMutex<bool>,
    release: Condvar,
}

impl Gate {
    pub(crate) async fn wait_started(&self) {
        self.started.notified().await;
    }

    pub(crate) fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.release.notify_all();
    }
}

pub(crate) struct Sink {
    records: StdMutex<Vec<Record>>,
    fail: bool,
    gate: Option<Arc<Gate>>,
}

impl Sink {
    pub(crate) fn new(fail: bool) -> Self {
        Self {
            records: StdMutex::new(Vec::new()),
            fail,
            gate: None,
        }
    }

    pub(crate) fn gated() -> (Self, Arc<Gate>) {
        let gate = Arc::new(Gate {
            started: Notify::new(),
            released: StdMutex::new(false),
            release: Condvar::new(),
        });
        (
            Self {
                records: StdMutex::new(Vec::new()),
                fail: false,
                gate: Some(gate.clone()),
            },
            gate,
        )
    }

    pub(crate) fn records(&self) -> Vec<Record> {
        self.records.lock().unwrap().clone()
    }
}

impl AuditSink for Sink {
    fn record(&self, _: VaultOp, _: &str, _: &AuditMetadata<'_>) -> AuditResult<()> {
        Ok(())
    }

    fn record_protected(
        &self,
        op: ProtectedOp,
        subject: &str,
        metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        self.records.lock().unwrap().push(Record {
            op,
            subject: subject.into(),
            session_id: metadata.session_id.map(str::to_owned),
            cwd: metadata.cwd.map(PathBuf::from),
            agent_name: metadata.agent_name.map(str::to_owned),
            depth: metadata.depth,
            generation: metadata.connection_generation,
            workflow_run_id: metadata.workflow_run_id.map(str::to_owned),
            workflow_node_id: metadata.workflow_node_id.map(str::to_owned),
            workflow_attempt_id: metadata.workflow_attempt_id.map(str::to_owned),
            workflow_phase: metadata.workflow_phase.map(str::to_owned),
            spawn_target: metadata.spawn_target.map(str::to_owned),
            model: metadata.model.map(str::to_owned),
            permission_mode: metadata.permission_mode.map(str::to_owned),
            decision_mode: metadata.decision_mode.map(str::to_owned),
            sandbox_policy: metadata.sandbox_policy.map(str::to_owned),
        });
        if let Some(gate) = &self.gate {
            gate.started.notify_one();
            let mut released = gate.released.lock().unwrap();
            while !*released {
                released = gate.release.wait(released).unwrap();
            }
        }
        if self.fail {
            Err(AuditError::Serialization("spawn sink failed".into()))
        } else {
            Ok(())
        }
    }
}

pub(crate) async fn agent_fixture(
    sink: Option<Arc<dyn AuditSink>>,
) -> (Arc<Mutex<Hub>>, PreparedSpawn, AgentExecutionRef) {
    let (events, _event_rx) = mpsc::channel(8);
    let mut hub = Hub::new(events);
    let (transport, _peer) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(transport).into_listening();
    let execution = hub
        .registry
        .register_connection_with_parent_execution("parent", connection, None, None, None)
        .unwrap();
    let mut facts = AgentRuntimeFacts::root(PathBuf::from("/tmp"), SpawnAuthority::default());
    facts.session_id = Some("session-spawn".into());
    facts.workflow_permission_causation = Some(WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_spawn"),
        node_id: WorkflowNodeId::new("wnode_spawn"),
        attempt_id: WorkflowAttemptId::new("watt_spawn"),
    });
    assert!(hub.registry.set_runtime_facts(&execution, facts));
    if let Some(sink) = sink {
        hub.set_protected_audit(sink);
    }
    let prepared = PreparedSpawn {
        name: "child".into(),
        request_lease: SpawnRequestLease::Agent(execution.clone()),
        cwd: PathBuf::from("/tmp"),
        prompt: Some("not audited".into()),
        parent: Some(QualifiedAddress::local("parent")),
        parent_execution: Some(execution.clone()),
        authority: SpawnAuthority::default(),
        agent_type: None,
        depth: 1,
        fork_context: Some(serde_json::json!({"not": "audited"})),
        workflow_permission_causation: None,
        workflow_attempt_capability: None,
        workflow_completion_result_limit: None,
        notify_parent_on_completion: true,
        root_cwd: PathBuf::from("/tmp"),
        root: "root".into(),
    };
    (Arc::new(Mutex::new(hub)), prepared, execution)
}

pub(crate) fn workflow_causation() -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_prepared"),
        node_id: WorkflowNodeId::new("wnode_prepared"),
        attempt_id: WorkflowAttemptId::new("watt_prepared"),
    }
}
