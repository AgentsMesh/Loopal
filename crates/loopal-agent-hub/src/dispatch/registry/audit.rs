use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use tokio::sync::Mutex;

use crate::dispatch::protected_audit_handler::{
    handle_permission_decision, handle_protected_effect,
};
use crate::hub::Hub;

use super::string_err_to_rpc;

pub fn register(builder: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let effect_hub = hub.clone();
    let builder = builder.register_fn(
        methods::HUB_AUDIT_PROTECTED_EFFECT.name,
        move |params, ctx| {
            let hub = effect_hub.clone();
            let agent = crate::dispatch::authorization::agent(ctx);
            Box::pin(async move {
                let agent = agent?;
                crate::dispatch::authorization::revalidate_agent(&hub, &agent).await?;
                handle_protected_effect(&hub, params, &agent)
                    .await
                    .map_err(string_err_to_rpc)
            })
        },
    );
    builder.register_fn(
        methods::HUB_AUDIT_PERMISSION_DECISION.name,
        move |params, ctx| {
            let hub = hub.clone();
            let agent = crate::dispatch::authorization::agent(ctx);
            Box::pin(async move {
                let agent = agent?;
                crate::dispatch::authorization::revalidate_agent(&hub, &agent).await?;
                handle_permission_decision(&hub, params, &agent)
                    .await
                    .map_err(string_err_to_rpc)
            })
        },
    )
}
