use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use serde_json::Value;
use tokio::sync::Mutex;

use loopal_hub_vault::{AuditContext, HubVaultService};
use loopal_protocol::{
    SecretCaller, SecretGetRequest, SecretGetResponse, SecretHealthRequest,
    SecretHealthResponse, SecretIpcError, SecretListNamesRequest, SecretListNamesResponse,
};
use loopal_secret_client::{ExposeSecret, SecretError};

use crate::hub::Hub;
use crate::spawn_registry::SpawnRegistry;

pub async fn handle_secret_get(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    from_agent: &str,
) -> Result<Value, String> {
    let req: SecretGetRequest = serde_json::from_value(params)
        .map_err(|e| format!("invalid secret_get params: {e}"))?;
    let (vault, spawn_registry) = resolve_deps(hub).await?;
    verify_caller(&spawn_registry, from_agent, &req.cwd, Some(&req.caller))
        .map_err(map_err)?;
    let cwd = PathBuf::from(&req.cwd);
    let plain = vault
        .get(&cwd, &req.name, audit_ctx(&req.caller))
        .await
        .map_err(map_err)?;
    serde_json::to_value(SecretGetResponse {
        plaintext: plain.expose_secret().to_string(),
    })
    .map_err(|e| e.to_string())
}

pub async fn handle_secret_list_names(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    from_agent: &str,
) -> Result<Value, String> {
    let req: SecretListNamesRequest = serde_json::from_value(params)
        .map_err(|e| format!("invalid secret_list_names params: {e}"))?;
    let (vault, spawn_registry) = resolve_deps(hub).await?;
    verify_caller(&spawn_registry, from_agent, &req.cwd, None).map_err(map_err)?;
    let names = vault
        .list_names(&PathBuf::from(&req.cwd))
        .await
        .map_err(map_err)?;
    serde_json::to_value(SecretListNamesResponse { names })
        .map_err(|e| e.to_string())
}

pub async fn handle_secret_health(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
) -> Result<Value, String> {
    let req: SecretHealthRequest = serde_json::from_value(params)
        .map_err(|e| format!("invalid secret_health params: {e}"))?;
    let (vault, _) = resolve_deps(hub).await?;
    let names = vault
        .list_names(&PathBuf::from(&req.cwd))
        .await
        .map_err(map_err)?;
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = names;
    serde_json::to_value(SecretHealthResponse {
        vault_count: 1,
        default_vault: "default".to_string(),
        last_op_ts: ts,
    })
    .map_err(|e| e.to_string())
}

async fn resolve_deps(
    hub: &Arc<Mutex<Hub>>,
) -> Result<(Arc<HubVaultService>, Arc<SpawnRegistry>), String> {
    let h = hub.lock().await;
    let vault = h
        .vault_service
        .clone()
        .ok_or_else(|| "vault service not initialized in Hub".to_string())?;
    let spawn_registry = h.spawn_registry.clone();
    Ok((vault, spawn_registry))
}

fn verify_caller(
    spawn_registry: &SpawnRegistry,
    from_agent: &str,
    cwd: &str,
    caller: Option<&SecretCaller>,
) -> Result<(), SecretError> {
    if let Some(c) = caller
        && c.agent_name != from_agent
    {
        return Err(SecretError::PermissionDenied);
    }
    if !spawn_registry.verify_vault_access(from_agent, std::path::Path::new(cwd)) {
        return Err(SecretError::PermissionDenied);
    }
    Ok(())
}

fn audit_ctx(caller: &SecretCaller) -> AuditContext {
    AuditContext {
        agent_name: caller.agent_name.clone(),
        depth: caller.depth,
        tool_name: caller.tool_name.clone(),
    }
}

fn map_err(e: SecretError) -> String {
    let ipc_err = match e {
        SecretError::SecretNotFound(name) => SecretIpcError::SecretNotFound { name },
        SecretError::VaultNotFound(p) => SecretIpcError::VaultNotFound {
            cwd: p.display().to_string(),
        },
        SecretError::PermissionDenied => SecretIpcError::PermissionDenied,
        SecretError::DecryptFailed(detail) => SecretIpcError::DecryptFailed { detail },
        SecretError::InvalidName(name) => SecretIpcError::InvalidName { name },
        SecretError::TemplateParse(detail) => SecretIpcError::TemplateParse { detail },
        SecretError::Ipc(detail) => SecretIpcError::Ipc { detail },
    };
    serde_json::to_string(&ipc_err).unwrap_or_else(|e| format!("ipc_encode_failed: {e}"))
}
