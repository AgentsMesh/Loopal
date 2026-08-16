//! Hub request dispatcher — routes incoming `hub/*` IPC requests.

use std::sync::Arc;

use loopal_ipc::{Dispatcher, DispatcherBuilder, RpcError, jsonrpc};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::hub::Hub;

pub(crate) mod authorization;
mod authorization_policy;
#[cfg(test)]
mod authorization_tests;
#[cfg(test)]
mod authorization_workflow_tests;
#[cfg(test)]
mod authorization_workflow_worker_tests;
mod cross_hub_forward;
mod cross_hub_spawn_admission;
pub(crate) mod dispatch_handlers;
#[cfg(test)]
#[path = "fallback_tests.rs"]
mod fallback_tests;
mod mcp_handlers;
mod protected_audit_handler;
mod registry;
#[cfg(test)]
#[path = "relay_response_coverage_tests.rs"]
mod relay_response_coverage_tests;
pub(crate) mod relay_response_handlers;
mod route_handler;
mod secret_handlers;
mod shutdown_handler;
mod skill_routing;
mod spawn_authority;
mod spawn_authority_fields;
#[cfg(test)]
#[path = "spawn_authority_tests.rs"]
mod spawn_authority_tests;
mod spawn_parent_policy;
#[doc(hidden)]
pub mod spawn_prepare;
#[cfg(test)]
#[path = "spawn_prepare_tests.rs"]
mod spawn_prepare_tests;
mod spawn_routing;
mod status_handler;
mod topology_handlers;
mod wait_handler;

pub fn build_hub_dispatcher(hub: Arc<Mutex<Hub>>) -> Dispatcher {
    let b = registry::register_all(DispatcherBuilder::new(), hub.clone());

    // reason: meta/* methods are forwarded to the MetaHub cluster via the
    // configured uplink — open-set protocol, only the prefix is fixed.
    b.fallback(move |method, params, _ctx| {
        let hub = hub.clone();
        let method = method.to_string();
        Box::pin(async move { handle_fallback(hub, method, params).await })
    })
    .build()
}

async fn handle_fallback(
    hub: Arc<Mutex<Hub>>,
    method: String,
    params: Value,
) -> Result<Value, RpcError> {
    if !method.starts_with("meta/") {
        return Err(RpcError::Remote {
            code: jsonrpc::METHOD_NOT_FOUND,
            message: format!("unknown hub method: {method}"),
            data: None,
        });
    }
    let uplink = hub.lock().await.uplink.clone();
    let Some(ul) = uplink else {
        return Err(make_invalid_request(
            "not connected to MetaHub cluster".into(),
        ));
    };
    let resp = ul
        .connection()
        .send_request(&method, params)
        .await
        .map_err(|e| make_invalid_request(format!("{method} via uplink failed: {e}")))?;
    if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
        Err(make_invalid_request(format!("{method} error: {msg}")))
    } else {
        Ok(resp)
    }
}

pub(crate) fn make_invalid_request(message: String) -> RpcError {
    RpcError::Remote {
        code: jsonrpc::INVALID_REQUEST,
        message,
        data: None,
    }
}

/// Dispatch via a caller-supplied `Dispatcher` instance.
///
/// Use this on the hot path (`agent_io_loop`, `tcp_ui_io_loop`) where the
/// dispatcher is built once and shared. Building it costs ~20 register_fn +
/// Arc allocations.
pub(crate) async fn dispatch_hub_request_with_principal(
    hub: &Arc<Mutex<Hub>>,
    dispatcher: &Dispatcher,
    method: &str,
    params: Value,
    principal: Arc<crate::request_principal::HubRequestPrincipal>,
) -> Result<Value, String> {
    let ctx = authorization::authorize(hub, method, principal)
        .await
        .map_err(|error| error.to_string())?;
    dispatcher
        .dispatch(method, params, &ctx)
        .await
        .map_err(|e| {
            if let RpcError::Remote { message, .. } = &e {
                message.clone()
            } else {
                e.to_string()
            }
        })
}

/// One-shot internal dispatch for tests and coordinator-owned paths.
#[doc(hidden)]
pub async fn dispatch_hub_request(
    hub: &Arc<Mutex<Hub>>,
    method: &str,
    params: Value,
    _from: String,
) -> Result<Value, String> {
    let dispatcher = build_hub_dispatcher(hub.clone());
    dispatch_hub_request_with_principal(
        hub,
        &dispatcher,
        method,
        params,
        Arc::new(crate::request_principal::HubRequestPrincipal::Internal),
    )
    .await
}
