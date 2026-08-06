use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_protocol::AgentEvent;

pub struct HubBuilt {
    pub(crate) hub: Arc<Mutex<Hub>>,
    pub(crate) event_rx: mpsc::Receiver<AgentEvent>,
}

pub struct ListenerBound {
    pub(crate) hub: Arc<Mutex<Hub>>,
    pub(crate) event_rx: mpsc::Receiver<AgentEvent>,
    pub(crate) listener_addr: String,
    pub(crate) hub_token: String,
}

pub struct DispatcherReady {
    pub(crate) hub: Arc<Mutex<Hub>>,
    pub(crate) event_rx: mpsc::Receiver<AgentEvent>,
    pub(crate) hub_token: String,
    pub(crate) dispatcher: Arc<loopal_ipc::Dispatcher>,
}

pub struct AgentSpawned {
    pub(crate) hub: Arc<Mutex<Hub>>,
    pub(crate) event_rx: mpsc::Receiver<AgentEvent>,
    pub(crate) hub_token: String,
    pub(crate) agent_proc: loopal_agent_client::AgentProcess,
    pub(crate) client_conn:
        Arc<loopal_ipc::connection::Connection<loopal_ipc::connection::Listening>>,
}

/// The Hub event reducer is live and the root agent has not been started.
///
/// Interactive frontends must install their real UI lease while holding this
/// state, then consume it through `start_root_agent`.
pub struct RootPending {
    pub(crate) hub: Arc<Mutex<Hub>>,
    pub(crate) hub_token: String,
    pub(crate) agent_proc: loopal_agent_client::AgentProcess,
    pub(crate) client_conn:
        Arc<loopal_ipc::connection::Connection<loopal_ipc::connection::Listening>>,
}

pub struct Ready {
    pub(crate) hub: Arc<Mutex<Hub>>,
    pub(crate) hub_token: String,
    pub(crate) agent_proc: loopal_agent_client::AgentProcess,
    pub(crate) root_session_id: String,
}
