use std::sync::Arc;

use loopal_ipc::{DispatcherBuilder, protocol::methods};
use loopal_workspace::git_types::{CreateWorktreeParams, RemoveWorktreeParams};
use loopal_workspace::types::{
    SearchParams, WorkspaceParams, WorkspacePathParams, WriteFileParams,
};
use tokio::sync::Mutex;

use crate::hub::Hub;

use super::{decode, encode, workspace_err, workspace_for_ui};

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(
        methods::WORKSPACE_LIST_DIRECTORY.name,
        move |params, ctx| {
            let h = h.clone();
            let from = ctx.from.clone();
            Box::pin(async move {
                let service = workspace_for_ui(&h, &from).await?;
                encode(
                    service
                        .list_directory(decode(params)?)
                        .await
                        .map_err(workspace_err)?,
                )
            })
        },
    );
    let h = hub.clone();
    let b = b.register_fn(methods::WORKSPACE_READ_FILE.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let service = workspace_for_ui(&h, &from).await?;
            let input: WorkspacePathParams = decode(params)?;
            encode(service.read_file(input).await.map_err(workspace_err)?)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::WORKSPACE_WRITE_FILE.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let service = workspace_for_ui(&h, &from).await?;
            let input: WriteFileParams = decode(params)?;
            encode(service.write_file(input).await.map_err(workspace_err)?)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::WORKSPACE_SEARCH.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let service = workspace_for_ui(&h, &from).await?;
            let input: SearchParams = decode(params)?;
            encode(service.search(input).await.map_err(workspace_err)?)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::WORKSPACE_GIT_STATUS.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let service = workspace_for_ui(&h, &from).await?;
            let input: WorkspaceParams = decode(params)?;
            encode(service.git_status(input).await.map_err(workspace_err)?)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::WORKSPACE_GIT_DIFF.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let service = workspace_for_ui(&h, &from).await?;
            let input: WorkspacePathParams = decode(params)?;
            encode(service.git_diff(input).await.map_err(workspace_err)?)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(
        methods::WORKSPACE_LIST_WORKTREES.name,
        move |params, ctx| {
            let h = h.clone();
            let from = ctx.from.clone();
            Box::pin(async move {
                let service = workspace_for_ui(&h, &from).await?;
                let input: WorkspaceParams = decode(params)?;
                encode(service.list_worktrees(input).await.map_err(workspace_err)?)
            })
        },
    );
    let h = hub.clone();
    let b = b.register_fn(methods::WORKSPACE_GIT_STAGE.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let service = workspace_for_ui(&h, &from).await?;
            let input: WorkspacePathParams = decode(params)?;
            service.git_stage(input).await.map_err(workspace_err)?;
            encode(serde_json::json!({ "ok": true }))
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::WORKSPACE_GIT_UNSTAGE.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            let service = workspace_for_ui(&h, &from).await?;
            let input: WorkspacePathParams = decode(params)?;
            service.git_unstage(input).await.map_err(workspace_err)?;
            encode(serde_json::json!({ "ok": true }))
        })
    });
    let h = hub.clone();
    let b = b.register_fn(
        methods::WORKSPACE_CREATE_WORKTREE.name,
        move |params, ctx| {
            let h = h.clone();
            let from = ctx.from.clone();
            Box::pin(async move {
                let service = workspace_for_ui(&h, &from).await?;
                let input: CreateWorktreeParams = decode(params)?;
                encode(
                    service
                        .create_worktree(input)
                        .await
                        .map_err(workspace_err)?,
                )
            })
        },
    );
    b.register_fn(
        methods::WORKSPACE_REMOVE_WORKTREE.name,
        move |params, ctx| {
            let h = hub.clone();
            let from = ctx.from.clone();
            Box::pin(async move {
                let service = workspace_for_ui(&h, &from).await?;
                let input: RemoveWorktreeParams = decode(params)?;
                service
                    .remove_worktree(input)
                    .await
                    .map_err(workspace_err)?;
                encode(serde_json::json!({ "ok": true }))
            })
        },
    )
}
