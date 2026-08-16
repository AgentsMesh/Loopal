use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use tokio::sync::Mutex;

use crate::dispatch::spawn_routing::{handle_spawn_agent, handle_spawn_remote_agent};
use crate::dispatch::wait_handler::handle_wait_agent;
use crate::hub::Hub;

use super::string_err_to_rpc;

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_SPAWN_AGENT.name, move |params, ctx| {
        let h = h.clone();
        Box::pin(async move {
            let agent = crate::dispatch::authorization::agent(ctx)?;
            crate::dispatch::authorization::revalidate_agent(&h, &agent).await?;
            handle_spawn_agent(&h, params, &agent)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_SPAWN_REMOTE_AGENT.name, move |params, ctx| {
        let h = h.clone();
        Box::pin(async move {
            let meta = crate::dispatch::authorization::trusted_meta(ctx)?;
            handle_spawn_remote_agent(&h, params, &meta)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    b.register_fn(methods::HUB_WAIT_AGENT.name, move |params, _ctx| {
        let h = h.clone();
        Box::pin(async move {
            handle_wait_agent(&h, params)
                .await
                .map_err(string_err_to_rpc)
        })
    })
}
