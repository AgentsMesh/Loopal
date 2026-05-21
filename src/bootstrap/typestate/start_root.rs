use std::time::Duration;

use super::states::{AgentSpawned, Ready};

// reason: layered timeouts let the innermost failure surface first.
// proxy(HUB_RPC_BUDGET=8s) < start_agent(20s) < HANDSHAKE(30s).
const START_AGENT_TIMEOUT: Duration = Duration::from_secs(20);

impl AgentSpawned {
    pub async fn start_root_agent(
        self,
        params: &loopal_agent_client::StartAgentParams,
    ) -> anyhow::Result<Ready> {
        let AgentSpawned {
            hub,
            event_rx,
            hub_token,
            agent_proc,
            client_conn,
        } = self;
        let root_session_id = loopal_agent_client::AgentClient::start_agent_on(
            &client_conn,
            params,
            START_AGENT_TIMEOUT,
        )
        .await?;
        drop(client_conn);
        Ok(Ready {
            hub,
            event_rx,
            hub_token,
            agent_proc,
            root_session_id,
        })
    }
}
