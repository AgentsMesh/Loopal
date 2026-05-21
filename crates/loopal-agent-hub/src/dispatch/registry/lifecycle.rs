use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use tokio::sync::Mutex;

use crate::dispatch::dispatch_handlers::{
    handle_control, handle_interrupt, handle_list_agents, handle_route, handle_shutdown_agent,
};
use crate::dispatch::shutdown_handler::handle_hub_shutdown;
use crate::hub::Hub;

use super::string_err_to_rpc;

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_ROUTE.name, move |params, _ctx| {
        let h = h.clone();
        Box::pin(async move { handle_route(&h, params).await.map_err(string_err_to_rpc) })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_LIST_AGENTS.name, move |_params, _ctx| {
        let h = h.clone();
        Box::pin(async move { handle_list_agents(&h).await.map_err(string_err_to_rpc) })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_CONTROL.name, move |params, _ctx| {
        let h = h.clone();
        Box::pin(async move { handle_control(&h, params).await.map_err(string_err_to_rpc) })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_INTERRUPT.name, move |params, _ctx| {
        let h = h.clone();
        Box::pin(async move {
            handle_interrupt(&h, params)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_SHUTDOWN_AGENT.name, move |params, _ctx| {
        let h = h.clone();
        Box::pin(async move {
            handle_shutdown_agent(&h, params)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    b.register_fn(methods::HUB_SHUTDOWN.name, move |_params, _ctx| {
        let h = h.clone();
        Box::pin(async move { handle_hub_shutdown(&h).await.map_err(string_err_to_rpc) })
    })
}
