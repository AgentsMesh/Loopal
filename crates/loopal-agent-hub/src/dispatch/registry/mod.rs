use std::sync::Arc;

use loopal_ipc::{DispatcherBuilder, RpcError, jsonrpc};
use tokio::sync::Mutex;

use crate::hub::Hub;

mod lifecycle;
mod mcp;
mod relay;
mod secret;
mod spawn;
mod topology;

pub(super) fn string_err_to_rpc(e: String) -> RpcError {
    RpcError::Remote {
        code: jsonrpc::INVALID_REQUEST,
        message: e,
        data: None,
    }
}

pub fn register_all(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let b = lifecycle::register(b, hub.clone());
    let b = mcp::register(b, hub.clone());
    let b = secret::register(b, hub.clone());
    let b = spawn::register(b, hub.clone());
    let b = topology::register(b, hub.clone());
    relay::register(b, hub)
}
