use super::states::{AgentSpawned, RootPending};

impl AgentSpawned {
    /// Make Hub event reduction and broadcast live before root startup.
    pub fn start_event_loop(self) -> RootPending {
        let AgentSpawned {
            hub,
            event_rx,
            hub_token,
            agent_proc,
            client_conn,
        } = self;
        loopal_agent_hub::start_event_loop(hub.clone(), event_rx);
        RootPending {
            hub,
            hub_token,
            agent_proc,
            client_conn,
            workflow_runtime: None,
        }
    }
}
