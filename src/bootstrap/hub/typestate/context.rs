use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_agent_hub::Hub;

pub struct BootstrapContext {
    pub hub: Arc<Mutex<Hub>>,
    pub agent_proc: loopal_agent_client::AgentProcess,
    pub root_session_id: String,
    pub hub_token: String,
}

pub struct HubAliveInfo {
    pub addr: String,
    pub token: String,
}
