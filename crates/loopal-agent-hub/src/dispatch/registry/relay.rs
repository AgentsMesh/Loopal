use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use tokio::sync::Mutex;

use crate::dispatch::relay_response_handlers::{
    handle_permission_response, handle_plan_approval_response, handle_question_response,
};
use crate::hub::Hub;

use super::string_err_to_rpc;

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(
        methods::HUB_PERMISSION_RESPONSE.name,
        move |params, _ctx| {
            let h = h.clone();
            Box::pin(async move {
                handle_permission_response(&h, params)
                    .await
                    .map_err(string_err_to_rpc)
            })
        },
    );
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_QUESTION_RESPONSE.name, move |params, _ctx| {
        let h = h.clone();
        Box::pin(async move {
            handle_question_response(&h, params)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(
        methods::HUB_PLAN_APPROVAL_RESPONSE.name,
        move |params, _ctx| {
            let h = h.clone();
            Box::pin(async move {
                handle_plan_approval_response(&h, params)
                    .await
                    .map_err(string_err_to_rpc)
            })
        },
    );
    b.register_fn(methods::HUB_REMOTE_RELAY.name, move |params, ctx| {
        let hub = hub.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            crate::remote_relay::handle(&hub, params, &from)
                .await
                .map_err(string_err_to_rpc)
        })
    })
}
