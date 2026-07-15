use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use tokio::sync::Mutex;

use crate::Hub;
use crate::uplink_connection::{JoinMetaHubParams, connect, disconnect};

use super::{decode, string_err_to_rpc};

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let target = hub.clone();
    let b = b.register_fn(methods::HUB_JOIN_META.name, move |params, ctx| {
        let target = target.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            require_ui(&target, &from).await?;
            let input: JoinMetaHubParams = decode(params)?;
            connect(&target, &input.address, &input.token, &input.hub_name)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    b.register_fn(methods::HUB_LEAVE_META.name, move |_params, ctx| {
        let target = hub.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            require_ui(&target, &from).await?;
            disconnect(&target).await.map_err(string_err_to_rpc)
        })
    })
}

async fn require_ui(hub: &Arc<Mutex<Hub>>, from: &str) -> Result<(), loopal_ipc::RpcError> {
    if hub.lock().await.ui.is_ui_client(from) {
        Ok(())
    } else {
        Err(string_err_to_rpc(
            "MetaHub lifecycle requires a UI client".into(),
        ))
    }
}
