use std::fmt;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::sync::Mutex;

use super::*;

pub struct Hub;
pub struct ClientConnection;

#[derive(Default)]
pub struct StartAgentParams {
    pub resume: Option<String>,
    pub session_id: Option<String>,
}

pub struct AgentClient;

impl AgentClient {
    pub async fn start_agent_on(
        _connection: &Arc<ClientConnection>,
        params: &StartAgentParams,
        _timeout: Duration,
    ) -> Result<String> {
        if fails(FAIL_START) {
            return Err(Error("synthetic Agent start failure".into()));
        }
        if fails(MISMATCH_SESSION) {
            return Ok("different-session".into());
        }
        Ok(params
            .resume
            .clone()
            .or_else(|| params.session_id.clone())
            .expect("bootstrap must bind a session id"))
    }
}

pub struct AgentProcess;

impl AgentProcess {
    pub async fn shutdown(self) -> Result<()> {
        PROCESS_SHUTDOWNS.fetch_add(1, Ordering::SeqCst);
        if fails(FAIL_PROCESS_SHUTDOWN) {
            Err(Error("synthetic Agent shutdown failure".into()))
        } else {
            Ok(())
        }
    }
}

pub struct QualifiedAddress;

impl QualifiedAddress {
    pub fn local(_agent: impl Into<String>) -> Self {
        Self
    }
}

pub const ROOT_AGENT_NAME: &str = "main";

pub struct Uuid;

impl Uuid {
    pub fn new_v4() -> Self {
        Self
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("generated-session")
    }
}

pub mod agent_io {
    use super::*;

    pub async fn bind_managed_root_session_id(
        _hub: &Arc<Mutex<Hub>>,
        _connection: &Arc<ClientConnection>,
        _session_id: &str,
    ) -> Result<()> {
        if fails(FAIL_BIND) {
            Err(Error("synthetic bind failure".into()))
        } else {
            Ok(())
        }
    }
}

pub mod workflow {
    use super::*;

    pub struct WorkflowOwner;

    impl WorkflowOwner {
        pub fn new(_session_id: String, _address: QualifiedAddress) -> Self {
            Self
        }
    }

    pub struct WorkflowRuntime;

    impl WorkflowRuntime {
        pub async fn recover_and_admit(&mut self, _owner: WorkflowOwner) -> Result<usize> {
            RECOVERIES.fetch_add(1, Ordering::SeqCst);
            if fails(FAIL_RECOVERY) {
                Err(Error("synthetic recovery failure".into()))
            } else {
                Ok(0)
            }
        }

        pub async fn activate_terminal_deliveries(&self) -> Result<()> {
            ACTIVATIONS.fetch_add(1, Ordering::SeqCst);
            if fails(FAIL_ACTIVATION) {
                Err(Error("synthetic activation failure".into()))
            } else {
                Ok(())
            }
        }

        pub async fn shutdown(self) -> Result<()> {
            RUNTIME_SHUTDOWNS.fetch_add(1, Ordering::SeqCst);
            if fails(FAIL_RUNTIME_SHUTDOWN) {
                Err(Error("synthetic runtime shutdown failure".into()))
            } else {
                Ok(())
            }
        }
    }
}

pub mod states {
    use super::*;
    use crate::workflow::WorkflowRuntime;

    pub struct RootPending {
        pub(crate) hub: Arc<Mutex<Hub>>,
        pub(crate) hub_token: String,
        pub(crate) agent_proc: AgentProcess,
        pub(crate) client_conn: Arc<ClientConnection>,
        pub(crate) workflow_runtime: Option<WorkflowRuntime>,
    }

    pub struct Ready {
        pub(crate) hub: Arc<Mutex<Hub>>,
        pub(crate) hub_token: String,
        pub(crate) agent_proc: AgentProcess,
        pub(crate) root_session_id: String,
        pub(crate) workflow_runtime: Option<WorkflowRuntime>,
    }
}
