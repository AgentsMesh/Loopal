use std::path::PathBuf;
use std::sync::Arc;

use loopal_ipc::{
    DispatcherBuilder, RpcError,
    protocol::methods::{
        self, DesktopDeleteSkillParams, DesktopGetSkillParams, DesktopListSkillsParams,
        DesktopUpsertSkillParams,
    },
};
use tokio::sync::Mutex;

use crate::hub::Hub;

use super::{
    decode, desktop_skill_store, encode, settings_context_for_ui, string_err_to_rpc, workspace_err,
};

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(methods::DESKTOP_LIST_SKILLS.name, move |params, ctx| {
        let hub = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let input: DesktopListSkillsParams = decode(params)?;
            let (root, user_dir) = context(&hub, &from, &input.workspace_id).await?;
            blocking(move || desktop_skill_store::list(&root, &user_dir, input.workspace_id)).await
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::DESKTOP_GET_SKILL.name, move |params, ctx| {
        let hub = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let input: DesktopGetSkillParams = decode(params)?;
            let (root, user_dir) = context(&hub, &from, &input.workspace_id).await?;
            blocking(move || {
                desktop_skill_store::get(&root, &user_dir, input.workspace_id, &input.name)
            })
            .await
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::DESKTOP_UPSERT_SKILL.name, move |params, ctx| {
        let hub = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let input: DesktopUpsertSkillParams = decode(params)?;
            let (root, user_dir) = context(&hub, &from, &input.workspace_id).await?;
            blocking(move || {
                desktop_skill_store::upsert(
                    &root,
                    &user_dir,
                    input.workspace_id,
                    &input.name,
                    &input.description,
                    &input.body,
                    input.expected_revision.as_deref(),
                )
            })
            .await
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::DESKTOP_DELETE_SKILL.name, move |params, ctx| {
        let hub = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let input: DesktopDeleteSkillParams = decode(params)?;
            let (root, user_dir) = context(&hub, &from, &input.workspace_id).await?;
            blocking(move || {
                desktop_skill_store::delete(
                    &root,
                    &user_dir,
                    input.workspace_id,
                    &input.name,
                    &input.expected_revision,
                )
            })
            .await
        })
    });
    b.register_fn(methods::DESKTOP_LIST_PLUGINS.name, move |params, ctx| {
        let hub = hub.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let input: DesktopListSkillsParams = decode(params)?;
            let (_, user_dir) = context(&hub, &from, &input.workspace_id).await?;
            blocking(move || desktop_skill_store::plugins(&user_dir, input.workspace_id)).await
        })
    })
}

async fn context(
    hub: &Arc<Mutex<Hub>>,
    from: &str,
    workspace_id: &str,
) -> Result<(PathBuf, PathBuf), RpcError> {
    let (service, user_dir) = settings_context_for_ui(hub, from).await?;
    service
        .require_workspace(workspace_id)
        .map_err(workspace_err)?;
    Ok((service.root().to_path_buf(), user_dir))
}

async fn blocking<T: serde::Serialize + Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<serde_json::Value, RpcError> {
    let value = tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| string_err_to_rpc(error.to_string()))?
        .map_err(string_err_to_rpc)?;
    encode(value)
}
