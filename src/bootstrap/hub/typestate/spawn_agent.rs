use std::time::Duration;

use tokio::sync::oneshot;
use tracing::info;

use super::states::{AgentSpawned, DispatcherReady};

impl DispatcherReady {
    pub async fn spawn_agent_process(self) -> anyhow::Result<AgentSpawned> {
        let DispatcherReady {
            hub,
            event_rx,
            hub_token,
            dispatcher,
        } = self;
        let agent_proc = loopal_agent_client::AgentProcess::spawn(None).await?;
        let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
        client.initialize().await?;

        let (conn, incoming_rx) = client.into_parts();
        let (ready_tx, ready_rx) = oneshot::channel();
        loopal_agent_hub::agent_io::start_agent_io(
            hub.clone(),
            dispatcher,
            loopal_protocol::ROOT_AGENT_NAME,
            conn.clone(),
            incoming_rx,
            Some(ready_tx),
        );
        wait_consumer_ready(ready_rx).await?;
        info!(name = %loopal_protocol::ROOT_AGENT_NAME, "reverse-IPC channel draining");

        Ok(AgentSpawned {
            hub,
            event_rx,
            hub_token,
            agent_proc,
            client_conn: conn,
        })
    }
}

async fn wait_consumer_ready(rx: oneshot::Receiver<()>) -> anyhow::Result<()> {
    const WAIT: Duration = Duration::from_secs(2);
    match tokio::time::timeout(WAIT, rx).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err(anyhow::anyhow!(
            "agent IO task aborted before reverse-IPC channel became ready"
        )),
        Err(_) => Err(anyhow::anyhow!(
            "reverse-IPC channel did not become ready within {}s",
            WAIT.as_secs()
        )),
    }
}
