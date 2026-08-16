use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use tokio::sync::Mutex;

use crate::dispatch::secret_handlers::{
    handle_secret_get, handle_secret_health, handle_secret_list_names,
    handle_workflow_provider_secret_get,
};
use crate::hub::Hub;

use super::string_err_to_rpc;

pub fn register(builder: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let target = hub.clone();
    let builder = builder.register_fn(methods::HUB_SECRET_GET.name, move |params, ctx| {
        let target = target.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let agent = agent?;
            crate::dispatch::authorization::revalidate_agent(&target, &agent).await?;
            handle_secret_get(&target, params, &agent)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let target = hub.clone();
    let builder = builder.register_fn(
        methods::HUB_WORKFLOW_PROVIDER_SECRET_GET.name,
        move |params, ctx| {
            let target = target.clone();
            let agent = crate::dispatch::authorization::agent(ctx);
            Box::pin(async move {
                let agent = agent?;
                crate::dispatch::authorization::revalidate_agent(&target, &agent).await?;
                handle_workflow_provider_secret_get(&target, params, &agent)
                    .await
                    .map_err(string_err_to_rpc)
            })
        },
    );
    let target = hub.clone();
    let builder = builder.register_fn(methods::HUB_SECRET_LIST_NAMES.name, move |params, ctx| {
        let target = target.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let agent = agent?;
            crate::dispatch::authorization::revalidate_agent(&target, &agent).await?;
            handle_secret_list_names(&target, params, &agent)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    builder.register_fn(methods::HUB_SECRET_HEALTH.name, move |params, ctx| {
        let target = hub.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let agent = agent?;
            crate::dispatch::authorization::revalidate_agent(&target, &agent).await?;
            handle_secret_health(&target, params, &agent)
                .await
                .map_err(string_err_to_rpc)
        })
    })
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
