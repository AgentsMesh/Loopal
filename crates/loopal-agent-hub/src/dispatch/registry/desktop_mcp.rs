use std::sync::Arc;

use loopal_ipc::{
    DispatcherBuilder,
    protocol::methods::{
        self, DesktopDeleteMcpServerParams, DesktopListMcpServersParams,
        DesktopUpsertMcpServerParams,
    },
};
use tokio::sync::Mutex;

use crate::hub::Hub;

use super::{
    decode, desktop_mcp_settings, encode, string_err_to_rpc, workspace_err, workspace_for_ui,
};

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(
        methods::DESKTOP_LIST_MCP_SERVERS.name,
        move |params, ctx| {
            let hub = h.clone();
            Box::pin(async move {
                let service = workspace_for_ui(&hub, &ctx.from).await?;
                let input: DesktopListMcpServersParams = decode(params)?;
                service
                    .require_workspace(&input.workspace_id)
                    .map_err(workspace_err)?;
                let root = service.root().to_path_buf();
                blocking(move || desktop_mcp_settings::load(&root, input.workspace_id)).await
            })
        },
    );
    let h = hub.clone();
    let b = b.register_fn(
        methods::DESKTOP_UPSERT_MCP_SERVER.name,
        move |params, ctx| {
            let hub = h.clone();
            Box::pin(async move {
                let service = workspace_for_ui(&hub, &ctx.from).await?;
                let input: DesktopUpsertMcpServerParams = decode(params)?;
                service
                    .require_workspace(&input.workspace_id)
                    .map_err(workspace_err)?;
                let root = service.root().to_path_buf();
                blocking(move || {
                    desktop_mcp_settings::upsert(&root, input.workspace_id, input.server)
                })
                .await
            })
        },
    );
    b.register_fn(
        methods::DESKTOP_DELETE_MCP_SERVER.name,
        move |params, ctx| {
            let hub = hub.clone();
            Box::pin(async move {
                let service = workspace_for_ui(&hub, &ctx.from).await?;
                let input: DesktopDeleteMcpServerParams = decode(params)?;
                service
                    .require_workspace(&input.workspace_id)
                    .map_err(workspace_err)?;
                let root = service.root().to_path_buf();
                blocking(move || {
                    desktop_mcp_settings::delete(&root, input.workspace_id, input.name)
                })
                .await
            })
        },
    )
}

async fn blocking(
    work: impl FnOnce() -> Result<loopal_ipc::protocol::methods::DesktopMcpServersResponse, String>
    + Send
    + 'static,
) -> Result<serde_json::Value, loopal_ipc::RpcError> {
    encode(
        tokio::task::spawn_blocking(work)
            .await
            .map_err(|error| string_err_to_rpc(error.to_string()))?
            .map_err(string_err_to_rpc)?,
    )
}
