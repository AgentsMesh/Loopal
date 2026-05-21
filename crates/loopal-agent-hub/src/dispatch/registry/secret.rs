use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use tokio::sync::Mutex;

use crate::dispatch::secret_handlers::{
    handle_secret_get, handle_secret_health, handle_secret_list_names,
};
use crate::hub::Hub;

use super::string_err_to_rpc;

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_SECRET_GET.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            handle_secret_get(&h, params, &from)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_SECRET_LIST_NAMES.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            handle_secret_list_names(&h, params, &from)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    b.register_fn(methods::HUB_SECRET_HEALTH.name, move |params, _ctx| {
        let h = h.clone();
        Box::pin(async move {
            handle_secret_health(&h, params)
                .await
                .map_err(string_err_to_rpc)
        })
    })
}
