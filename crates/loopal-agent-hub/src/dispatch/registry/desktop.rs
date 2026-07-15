use std::sync::Arc;

use loopal_ipc::{
    DispatcherBuilder,
    protocol::methods::{self, DesktopSettingsParams, DesktopUpdateSettingsParams},
};
use loopal_workspace::types::WorkspaceParams;
use tokio::sync::Mutex;

use crate::hub::Hub;

use super::{
    decode, desktop_settings, encode, settings_context_for_ui, string_err_to_rpc, workspace_err,
    workspace_for_ui,
};

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(methods::DESKTOP_LIST_SESSIONS.name, move |params, ctx| {
        let hub = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let service = workspace_for_ui(&hub, &from).await?;
            let input: WorkspaceParams = decode(params)?;
            encode(service.list_sessions(input).await.map_err(workspace_err)?)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::DESKTOP_GET_SETTINGS.name, move |params, ctx| {
        let hub = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let (service, user_dir) = settings_context_for_ui(&hub, &from).await?;
            let input: DesktopSettingsParams = decode(params)?;
            service
                .require_workspace(&input.workspace_id)
                .map_err(workspace_err)?;
            let root = service.root().to_path_buf();
            encode(
                tokio::task::spawn_blocking(move || {
                    desktop_settings::load(&root, &user_dir, input.workspace_id)
                })
                .await
                .map_err(|error| string_err_to_rpc(error.to_string()))?
                .map_err(string_err_to_rpc)?,
            )
        })
    });
    b.register_fn(methods::DESKTOP_UPDATE_SETTINGS.name, move |params, ctx| {
        let hub = hub.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let (service, user_dir) = settings_context_for_ui(&hub, &from).await?;
            let input: DesktopUpdateSettingsParams = decode(params)?;
            service
                .require_workspace(&input.workspace_id)
                .map_err(workspace_err)?;
            let root = service.root().to_path_buf();
            encode(
                tokio::task::spawn_blocking(move || {
                    desktop_settings::update(
                        &root,
                        &user_dir,
                        input.workspace_id,
                        input.settings,
                        input.provider_updates,
                    )
                })
                .await
                .map_err(|error| string_err_to_rpc(error.to_string()))?
                .map_err(string_err_to_rpc)?,
            )
        })
    })
}
