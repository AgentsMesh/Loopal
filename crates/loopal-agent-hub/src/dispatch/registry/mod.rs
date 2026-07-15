use std::sync::Arc;

use loopal_ipc::{DispatcherBuilder, RpcError, jsonrpc};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::hub::Hub;

mod desktop;
mod desktop_mcp;
mod desktop_mcp_layers;
mod desktop_mcp_projection;
mod desktop_mcp_secret_patches;
mod desktop_mcp_secret_policy;
mod desktop_mcp_settings;
mod desktop_mcp_url;
mod desktop_mcp_validation;
mod desktop_settings;
mod desktop_skill_projection;
mod desktop_skill_store;
mod desktop_skills;
mod lifecycle;
mod mcp;
mod metahub;
mod relay;
mod secret;
mod spawn;
mod topology;
mod workspace;

pub(super) fn string_err_to_rpc(e: String) -> RpcError {
    RpcError::Remote {
        code: jsonrpc::INVALID_REQUEST,
        message: e,
        data: None,
    }
}

pub fn register_all(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let b = lifecycle::register(b, hub.clone());
    let b = desktop::register(b, hub.clone());
    let b = desktop_mcp::register(b, hub.clone());
    let b = desktop_skills::register(b, hub.clone());
    let b = mcp::register(b, hub.clone());
    let b = metahub::register(b, hub.clone());
    let b = secret::register(b, hub.clone());
    let b = spawn::register(b, hub.clone());
    let b = topology::register(b, hub.clone());
    let b = workspace::register(b, hub.clone());
    relay::register(b, hub)
}

pub(super) async fn workspace_for_ui(
    hub: &Arc<Mutex<Hub>>,
    from: &str,
) -> Result<Arc<loopal_workspace::WorkspaceService>, RpcError> {
    let locked = hub.lock().await;
    if !locked.ui.is_ui_client(from) {
        return Err(string_err_to_rpc(
            "workspace methods require a UI client".into(),
        ));
    }
    locked
        .workspace
        .clone()
        .ok_or_else(|| string_err_to_rpc("workspace service unavailable".into()))
}

pub(super) async fn settings_context_for_ui(
    hub: &Arc<Mutex<Hub>>,
    from: &str,
) -> Result<(Arc<loopal_workspace::WorkspaceService>, std::path::PathBuf), RpcError> {
    let locked = hub.lock().await;
    if !locked.ui.is_ui_client(from) {
        return Err(string_err_to_rpc(
            "workspace methods require a UI client".into(),
        ));
    }
    let workspace = locked
        .workspace
        .clone()
        .ok_or_else(|| string_err_to_rpc("workspace service unavailable".into()))?;
    let user_dir = locked
        .user_config_dir
        .clone()
        .ok_or_else(|| string_err_to_rpc("Loopal user configuration is unavailable".into()))?;
    Ok((workspace, user_dir))
}

pub(super) fn decode<T: DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    serde_json::from_value(params).map_err(|error| string_err_to_rpc(error.to_string()))
}

pub(super) fn encode<T: Serialize>(value: T) -> Result<Value, RpcError> {
    serde_json::to_value(value).map_err(|error| string_err_to_rpc(error.to_string()))
}

pub(super) fn workspace_err(error: loopal_workspace::WorkspaceError) -> RpcError {
    RpcError::Remote {
        code: jsonrpc::INVALID_REQUEST,
        message: error.message,
        data: Some(serde_json::json!({ "code": error.code })),
    }
}
