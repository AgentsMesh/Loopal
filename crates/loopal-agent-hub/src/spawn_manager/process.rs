use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::transport::Transport;

use crate::types::RegisteredAgent;

const CHILD_START_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) type ProcessFuture = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;

pub(super) trait SpawnProcess: Send + 'static {
    fn transport(&self) -> Arc<dyn Transport>;
    fn shutdown(self) -> ProcessFuture;
    fn wait(self) -> ProcessFuture;
}

impl SpawnProcess for loopal_agent_client::AgentProcess {
    fn transport(&self) -> Arc<dyn Transport> {
        self.transport()
    }

    fn shutdown(self) -> ProcessFuture {
        Box::pin(async move { self.shutdown().await })
    }

    fn wait(self) -> ProcessFuture {
        Box::pin(async move { self.wait().await.map(|_| ()).map_err(Into::into) })
    }
}

trait WorkflowProcess: Send {
    fn shutdown(self: Box<Self>) -> ProcessFuture;
}

impl<P: SpawnProcess> WorkflowProcess for P {
    fn shutdown(self: Box<Self>) -> ProcessFuture {
        (*self).shutdown()
    }
}

pub(super) struct WorkflowProcessOwner(Box<dyn WorkflowProcess>);

impl WorkflowProcessOwner {
    pub(super) async fn shutdown(self) -> anyhow::Result<()> {
        self.0.shutdown().await
    }
}

pub(super) struct PreparedAgentProcess<P: SpawnProcess> {
    process: P,
    pub(super) connection: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    pub(super) registered: RegisteredAgent,
    session_id: String,
    start_params: loopal_agent_client::StartAgentParams,
    pub(super) name: String,
}

impl<P: SpawnProcess> PreparedAgentProcess<P> {
    pub(super) fn new(
        process: P,
        connection: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
        registered: RegisteredAgent,
        session_id: String,
        start_params: loopal_agent_client::StartAgentParams,
        name: String,
    ) -> Self {
        Self {
            process,
            connection,
            registered,
            session_id,
            start_params,
            name,
        }
    }

    pub(super) async fn activate(&self) -> Result<(), String> {
        activate(&self.connection, &self.start_params, &self.session_id).await
    }

    pub(super) fn into_workflow_parts(self) -> (WorkflowProcessOwner, PreparedControl) {
        let owner = WorkflowProcessOwner(Box::new(self.process));
        let control = PreparedControl {
            connection: self.connection,
            session_id: self.session_id,
            start_params: self.start_params,
        };
        (owner, control)
    }

    pub(super) async fn shutdown(self) {
        let _ = self.process.shutdown().await;
    }

    pub(super) fn spawn_wait(self) {
        tokio::spawn(async move {
            let _ = self.process.wait().await;
        });
    }
}

pub(super) struct PreparedControl {
    pub(super) connection: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    session_id: String,
    start_params: loopal_agent_client::StartAgentParams,
}

impl PreparedControl {
    pub(super) async fn activate(&self) -> Result<(), String> {
        activate(&self.connection, &self.start_params, &self.session_id).await
    }
}

async fn activate(
    connection: &Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    start_params: &loopal_agent_client::StartAgentParams,
    session_id: &str,
) -> Result<(), String> {
    let returned = loopal_agent_client::AgentClient::start_agent_on(
        connection,
        start_params,
        CHILD_START_TIMEOUT,
    )
    .await
    .map_err(|error| format!("agent/start failed: {error}"))?;
    (returned == session_id)
        .then_some(())
        .ok_or_else(|| "agent/start returned a different Hub-issued session id".into())
}
