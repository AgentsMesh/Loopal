use tracing::info;

mod bind_listener;
mod create_hub;
mod register_handlers;
mod spawn_agent;
mod start_event_loop;
mod start_root;
mod workflow_runtime;

pub mod context;
pub mod states;

pub use context::{BootstrapContext, HubAliveInfo};
pub use states::{HubBuilt, ListenerBound, Ready, RootPending};

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
            agent_proc: self.agent_proc,
            root_session_id: self.root_session_id,
            hub_token: self.hub_token,
            workflow_runtime: self.workflow_runtime,
        }
    }
}
