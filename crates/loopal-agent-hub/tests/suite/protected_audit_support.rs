use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use std::time::Duration;

use loopal_agent_hub::{Hub, hub_server};
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_vault_api::{AuditError, AuditMetadata, AuditResult, AuditSink, ProtectedOp, VaultOp};
use tokio::sync::{Mutex, Notify, mpsc};

#[derive(Clone, Debug)]
pub struct Record {
    pub op: ProtectedOp,
    pub subject: String,
    pub session_id: Option<String>,
    pub cwd: Option<PathBuf>,
    pub agent_name: Option<String>,
    pub depth: Option<u32>,
    pub connection_generation: Option<u64>,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub action_digest: Option<String>,
    pub schema_digest: Option<String>,
    pub intent_digest: Option<String>,
    pub workflow_run_id: Option<String>,
    pub workflow_node_id: Option<String>,
    pub workflow_attempt_id: Option<String>,
    pub decision: Option<String>,
    pub decision_source: Option<String>,
}

pub struct Gate {
    started: Notify,
    released: StdMutex<bool>,
    release: Condvar,
}

impl Gate {
    fn new() -> Self {
        Self {
            started: Notify::new(),
            released: StdMutex::new(false),
            release: Condvar::new(),
        }
    }

    pub async fn wait_started(&self) {
        self.started.notified().await;
    }

    pub fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.release.notify_all();
    }

    fn block(&self) {
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.release.wait(released).unwrap();
        }
    }
}

pub struct CapturingSink {
    records: StdMutex<Vec<Record>>,
    fail: bool,
    gate: Option<Arc<Gate>>,
}

impl CapturingSink {
    pub fn new(fail: bool) -> Self {
        Self {
            records: StdMutex::new(Vec::new()),
            fail,
            gate: None,
        }
    }

    pub fn gated() -> (Self, Arc<Gate>) {
        let gate = Arc::new(Gate::new());
        (
            Self {
                records: StdMutex::new(Vec::new()),
                fail: false,
                gate: Some(gate.clone()),
            },
            gate,
        )
    }

    pub fn records(&self) -> Vec<Record> {
        self.records.lock().unwrap().clone()
    }
}

impl AuditSink for CapturingSink {
    fn record(&self, _op: VaultOp, _key: &str, _metadata: &AuditMetadata<'_>) -> AuditResult<()> {
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
            connection_generation: metadata.connection_generation,
            tool_name: metadata.tool_name.map(str::to_owned),
            tool_call_id: metadata.tool_call_id.map(str::to_owned),
            action_digest: metadata.action_digest.map(str::to_owned),
            schema_digest: metadata.schema_digest.map(str::to_owned),
            intent_digest: metadata.intent_digest.map(str::to_owned),
            workflow_run_id: metadata.workflow_run_id.map(str::to_owned),
            workflow_node_id: metadata.workflow_node_id.map(str::to_owned),
            workflow_attempt_id: metadata.workflow_attempt_id.map(str::to_owned),
            decision: metadata.decision.map(str::to_owned),
            decision_source: metadata.decision_source.map(str::to_owned),
        });
        if let Some(gate) = &self.gate {
            gate.started.notify_one();
            gate.block();
        }
        if self.fail {
            Err(AuditError::Serialization("sink failed".into()))
        } else {
            Ok(())
        }
    }
}

pub struct Fixture {
    pub cwd: tempfile::TempDir,
    pub hub: Arc<Mutex<Hub>>,
    pub agent: Arc<Connection<Listening>>,
    _incoming: mpsc::Receiver<Incoming>,
}

pub async fn connected(sink: Option<Arc<dyn AuditSink>>) -> Fixture {
    let cwd = tempfile::tempdir().unwrap();
    let (event_tx, mut event_rx) = mpsc::channel(32);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let hub = Arc::new(Mutex::new(Hub::with_cwd(
        event_tx,
        cwd.path().to_path_buf(),
    )));
    if let Some(sink) = sink {
        hub.lock().await.set_protected_audit(sink);
    }
    let (agent, incoming) = hub_server::connect_local(hub.clone(), "worker");
    wait_registered(&hub, "worker").await;
    Fixture {
        cwd,
        hub,
        agent,
        _incoming: incoming,
    }
}

pub async fn wait_registered(hub: &Arc<Mutex<Hub>>, name: &str) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while hub
            .lock()
            .await
            .registry
            .get_agent_connection(name)
            .is_none()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("agent registration timed out");
}
