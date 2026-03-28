//! Hub request dispatcher — routes incoming `hub/*` IPC requests.

use std::sync::Arc;

use loopal_ipc::protocol::methods;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::hub::AgentHub;

mod dispatch_handlers;
mod topology_handlers;
mod wait_handler;

/// Dispatch a single `hub/*` request. Returns the JSON response value.
pub async fn dispatch_hub_request(
    hub: &Arc<Mutex<AgentHub>>,
    method: &str,
    params: Value,
    from_agent: String,
) -> Result<Value, String> {
    use dispatch_handlers::*;
    use topology_handlers::*;
    use wait_handler::handle_wait_agent;

    match method {
        m if m == methods::HUB_ROUTE.name => handle_route(hub, params).await,
        m if m == methods::HUB_LIST_AGENTS.name => handle_list_agents(hub).await,
        m if m == methods::HUB_CONTROL.name => handle_control(hub, params).await,
        m if m == methods::HUB_INTERRUPT.name => handle_interrupt(hub, params).await,
        m if m == methods::HUB_SHUTDOWN_AGENT.name => handle_shutdown_agent(hub, params).await,
        m if m == methods::HUB_SPAWN_AGENT.name => {
            handle_spawn_agent(hub, params, &from_agent).await
        }
        m if m == methods::HUB_WAIT_AGENT.name => handle_wait_agent(hub, params).await,
        m if m == methods::HUB_AGENT_INFO.name => handle_agent_info(hub, params).await,
        m if m == methods::HUB_TOPOLOGY.name => handle_topology(hub).await,
        _ => Err(format!("unknown hub method: {method}")),
    }
}
