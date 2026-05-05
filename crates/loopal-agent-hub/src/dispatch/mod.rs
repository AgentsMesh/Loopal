//! Hub request dispatcher — routes incoming `hub/*` IPC requests.

use std::sync::Arc;

use loopal_ipc::protocol::methods;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::hub::Hub;

mod cross_hub_forward;
mod dispatch_handlers;
mod relay_response_handlers;
mod spawn_prepare;
#[cfg(test)]
#[path = "spawn_prepare_test.rs"]
mod spawn_prepare_test;
mod spawn_routing;
mod status_handler;
mod topology_handlers;
mod wait_handler;

/// Dispatch a single `hub/*` request. Returns the JSON response value.
pub async fn dispatch_hub_request(
    hub: &Arc<Mutex<Hub>>,
    method: &str,
    params: Value,
    from_agent: String,
) -> Result<Value, String> {
    use dispatch_handlers::*;
    use relay_response_handlers::{handle_permission_response, handle_question_response};
    use spawn_routing::{handle_spawn_agent, handle_spawn_remote_agent};
    use status_handler::handle_status;
    use topology_handlers::*;
    use wait_handler::handle_wait_agent;

    match method {
        m if m == methods::HUB_ROUTE.name => handle_route(hub, params).await,
        m if m == methods::HUB_LIST_AGENTS.name => handle_list_agents(hub).await,
        m if m == methods::HUB_CONTROL.name => handle_control(hub, params).await,
        m if m == methods::HUB_INTERRUPT.name => handle_interrupt(hub, params).await,
        m if m == methods::HUB_SHUTDOWN_AGENT.name => handle_shutdown_agent(hub, params).await,
        m if m == methods::HUB_PERMISSION_RESPONSE.name => {
            handle_permission_response(hub, params).await
        }
        m if m == methods::HUB_QUESTION_RESPONSE.name => {
            handle_question_response(hub, params).await
        }
        m if m == methods::HUB_SPAWN_AGENT.name => {
            handle_spawn_agent(hub, params, &from_agent).await
        }
        m if m == methods::HUB_SPAWN_REMOTE_AGENT.name => {
            handle_spawn_remote_agent(hub, params, &from_agent).await
        }
        m if m == methods::HUB_WAIT_AGENT.name => handle_wait_agent(hub, params).await,
        m if m == methods::HUB_AGENT_INFO.name => handle_agent_info(hub, params).await,
        m if m == methods::HUB_TOPOLOGY.name => handle_topology(hub).await,
        m if m == methods::HUB_STATUS.name => handle_status(hub).await,
        // Forward meta/* methods to MetaHub via uplink
        m if m.starts_with("meta/") => {
            let uplink = hub.lock().await.uplink.clone();
            match uplink {
                Some(ul) => {
                    let resp = ul
                        .connection()
                        .send_request(method, params)
                        .await
                        .map_err(|e| format!("{method} via uplink failed: {e}"))?;
                    if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
                        Err(format!("{method} error: {msg}"))
                    } else {
                        Ok(resp)
                    }
                }
                None => Err("not connected to MetaHub cluster".to_string()),
            }
        }
        _ => Err(format!("unknown hub method: {method}")),
    }
}
