//! Hub request dispatcher — routes incoming `hub/*` IPC requests.

use std::sync::Arc;

use loopal_ipc::{Dispatcher, DispatcherBuilder, HandlerCtx, RpcError, jsonrpc};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::hub::Hub;

mod cross_hub_forward;
pub(crate) mod dispatch_handlers;
mod mcp_handlers;
mod registry;
pub(crate) mod relay_response_handlers;
mod secret_handlers;
mod shutdown_handler;
mod skill_routing;
mod spawn_parent_policy;
#[doc(hidden)]
pub mod spawn_prepare;
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

fn make_invalid_request(message: String) -> RpcError {
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
pub async fn dispatch_hub_request_with(
    dispatcher: &Dispatcher,
    method: &str,
    params: Value,
    from_agent: String,
) -> Result<Value, String> {
    let ctx = HandlerCtx::new(from_agent);
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

/// One-shot dispatch: build a fresh `Dispatcher` for this single call.
///
/// **For tests and cold paths only.** Production IO loops (agent_io,
/// ui_session, tcp_ui_io, uplink) hold an `Arc<Dispatcher>` and call
/// `dispatch_hub_request_with` instead.
#[doc(hidden)]
pub async fn dispatch_hub_request(
    hub: &Arc<Mutex<Hub>>,
    method: &str,
    params: Value,
    from_agent: String,
) -> Result<Value, String> {
    let dispatcher = build_hub_dispatcher(hub.clone());
    dispatch_hub_request_with(&dispatcher, method, params, from_agent).await
}
