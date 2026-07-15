use tracing::info;

mod bind_listener;
mod create_hub;
mod register_handlers;
mod spawn_agent;
mod start_root;

pub mod context;
pub mod states;

pub use context::{BootstrapContext, HubAliveInfo};
pub use states::{HubBuilt, ListenerBound, Ready};

impl ListenerBound {
    pub fn alive_info(&self) -> HubAliveInfo {
        HubAliveInfo {
            addr: self.listener_addr.clone(),
            token: self.hub_token.clone(),
        }
    }
}

impl Ready {
    pub fn into_context(self) -> BootstrapContext {
        info!("bootstrap: typestate chain complete, returning context");
        BootstrapContext {
            hub: self.hub,
            event_rx: self.event_rx,
            agent_proc: self.agent_proc,
            root_session_id: self.root_session_id,
            hub_token: self.hub_token,
        }
    }
}
