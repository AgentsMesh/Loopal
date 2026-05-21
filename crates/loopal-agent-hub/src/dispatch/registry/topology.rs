use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use tokio::sync::Mutex;

use crate::dispatch::status_handler::handle_status;
use crate::dispatch::topology_handlers::{handle_agent_info, handle_topology};
use crate::hub::Hub;

use super::string_err_to_rpc;

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_AGENT_INFO.name, move |params, _ctx| {
        let h = h.clone();
        Box::pin(async move {
            handle_agent_info(&h, params)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_TOPOLOGY.name, move |_params, _ctx| {
        let h = h.clone();
        Box::pin(async move { handle_topology(&h).await.map_err(string_err_to_rpc) })
    });
    let h = hub.clone();
    b.register_fn(methods::HUB_STATUS.name, move |_params, _ctx| {
        let h = h.clone();
        Box::pin(async move { handle_status(&h).await.map_err(string_err_to_rpc) })
    })
}
