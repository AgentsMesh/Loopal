use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use loopal_protocol::UiCapability;
use tokio::sync::Mutex;

use crate::dispatch::relay_response_handlers::{
    handle_permission_response, handle_plan_approval_response, handle_question_response,
};
use crate::hub::Hub;

use super::string_err_to_rpc;

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_PERMISSION_RESPONSE.name, move |params, ctx| {
        let h = h.clone();
        let ui = crate::dispatch::authorization::ui(ctx);
        Box::pin(async move {
            let ui = ui?;
            if !ui.capabilities.supports(UiCapability::Permission) {
                return Err(string_err_to_rpc(
                    "UI connection is not authorized for Permission responses".into(),
                ));
            }
            handle_permission_response(&h, params, &ui)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_QUESTION_RESPONSE.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            require_capability(&h, &from, UiCapability::Question)
                .await
                .map_err(string_err_to_rpc)?;
            handle_question_response(&h, params)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(
        methods::HUB_PLAN_APPROVAL_RESPONSE.name,
        move |params, ctx| {
            let h = h.clone();
            let from = ctx.from.clone();
            Box::pin(async move {
                require_capability(&h, &from, UiCapability::PlanApproval)
                    .await
                    .map_err(string_err_to_rpc)?;
                handle_plan_approval_response(&h, params)
                    .await
                    .map_err(string_err_to_rpc)
            })
        },
    );
    b.register_fn(methods::HUB_REMOTE_RELAY.name, move |params, ctx| {
        let hub = hub.clone();
        let trusted = crate::dispatch::authorization::trusted_meta(ctx);
        Box::pin(async move {
            let trusted = trusted?;
            crate::remote_relay::handle(&hub, params, &trusted)
                .await
                .map_err(string_err_to_rpc)
        })
    })
}

async fn require_capability(
    hub: &Arc<Mutex<Hub>>,
    lease_id: &str,
    capability: UiCapability,
) -> Result<(), String> {
    if hub
        .lock()
        .await
        .ui
        .client_has_capability(lease_id, capability)
    {
        Ok(())
    } else {
        Err(format!(
            "UI connection is not authorized for {capability:?} responses"
        ))
    }
}
