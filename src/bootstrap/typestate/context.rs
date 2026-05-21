use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_protocol::AgentEvent;

pub struct BootstrapContext {
    pub hub: Arc<Mutex<Hub>>,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub agent_proc: loopal_agent_client::AgentProcess,
    pub root_session_id: String,
    pub hub_token: String,
}

pub struct HubAliveInfo {
    pub addr: String,
    pub token: String,
}
