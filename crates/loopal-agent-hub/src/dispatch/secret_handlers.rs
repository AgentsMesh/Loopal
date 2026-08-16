use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use serde_json::Value;
use tokio::sync::Mutex;

use loopal_hub_vault::{AuditContext, HubVaultService};
use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{
    SecretCaller, SecretGetRequest, SecretGetResponse, SecretHealthRequest, SecretHealthResponse,
    SecretIpcError, SecretListNamesRequest, SecretListNamesResponse,
    WorkflowProviderSecretGetRequest,
};
use loopal_secret_client::{ExposeSecret, SecretError};

use crate::hub::Hub;
use crate::request_principal::AgentPrincipal;

pub async fn handle_secret_get(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    agent: &AgentPrincipal,
) -> Result<Value, String> {
    let request: SecretGetRequest =
        serde_json::from_value(params).map_err(|e| format!("invalid secret_get params: {e}"))?;
    verify_caller(agent, &request.caller).map_err(map_err)?;
    let (vault, cwd, redaction_seed) = authorize_cwd(hub, agent, &request.cwd).await?;
    let plain = vault
        .get(&cwd, &request.name, audit_ctx(agent, &request.caller))
        .await
        .map_err(map_err)?;
    redaction_seed
        .observe(&request.name, plain.clone())
        .map_err(|_| "final-sink redaction seed unavailable".to_string())?;
    serde_json::to_value(SecretGetResponse {
        plaintext: plain.expose_secret().to_string(),
    })
    .map_err(|error| error.to_string())
}

pub async fn handle_workflow_provider_secret_get(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    agent: &AgentPrincipal,
) -> Result<Value, String> {
    let request: WorkflowProviderSecretGetRequest = serde_json::from_value(params)
        .map_err(|error| format!("invalid workflow provider secret params: {error}"))?;
    verify_workflow_provider_authority(hub, agent, &request).await?;
    let caller = SecretCaller {
        agent_name: agent.execution.address.agent.clone(),
        depth: agent.depth,
        tool_name: Some("workflow_provider_config".into()),
    };
    let (vault, cwd, redaction_seed) = authorize_cwd(hub, agent, &request.cwd).await?;
    let plain = vault
        .get(&cwd, &request.name, audit_ctx(agent, &caller))
        .await
        .map_err(map_err)?;
    redaction_seed
        .observe(&request.name, plain.clone())
        .map_err(|_| "final-sink redaction seed unavailable".to_string())?;
    serde_json::to_value(SecretGetResponse {
        plaintext: plain.expose_secret().to_string(),
    })
    .map_err(|error| error.to_string())
}

pub async fn handle_secret_list_names(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    agent: &AgentPrincipal,
) -> Result<Value, String> {
    let request: SecretListNamesRequest = serde_json::from_value(params)
        .map_err(|e| format!("invalid secret_list_names params: {e}"))?;
    let (vault, cwd, _) = authorize_cwd(hub, agent, &request.cwd).await?;
    let names = vault.list_names(&cwd).await.map_err(map_err)?;
    serde_json::to_value(SecretListNamesResponse { names }).map_err(|error| error.to_string())
}

pub async fn handle_secret_health(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    agent: &AgentPrincipal,
) -> Result<Value, String> {
    let request: SecretHealthRequest =
        serde_json::from_value(params).map_err(|e| format!("invalid secret_health params: {e}"))?;
    let (vault, cwd, _) = authorize_cwd(hub, agent, &request.cwd).await?;
    vault.list_names(&cwd).await.map_err(map_err)?;
    let last_op_ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    serde_json::to_value(SecretHealthResponse {
        vault_count: 1,
        default_vault: "default".to_string(),
        last_op_ts,
    })
    .map_err(|error| error.to_string())
}

async fn authorize_cwd(
    hub: &Arc<Mutex<Hub>>,
    agent: &AgentPrincipal,
    requested_cwd: &str,
) -> Result<(Arc<HubVaultService>, PathBuf, FinalSinkRedactionSeed), String> {
    let requested = PathBuf::from(requested_cwd);
    let locked = hub.lock().await;
    if !locked.registry.owns_active_lease(&agent.execution)
        || !locked
            .spawn_registry
            .verify_vault_access_exact(&agent.execution, &requested)
    {
        return Err(map_err(SecretError::PermissionDenied));
    }
    let vault = locked
        .vault_service
        .clone()
        .ok_or_else(|| "vault service not initialized in Hub".to_string())?;
    Ok((vault, requested, locked.final_sink_redaction_seed()))
}

async fn verify_workflow_provider_authority(
    hub: &Arc<Mutex<Hub>>,
    agent: &AgentPrincipal,
    request: &WorkflowProviderSecretGetRequest,
) -> Result<(), String> {
    let locked = hub.lock().await;
    let facts = locked
        .registry
        .runtime_facts(&agent.execution)
        .filter(|_| locked.registry.owns_active_lease(&agent.execution))
        .ok_or_else(|| map_err(SecretError::PermissionDenied))?;
    let valid = facts.origin == crate::types::AgentOrigin::ManagedChild
        && facts.depth > 0
        && facts.parent.is_some()
        && agent.workflow_permission_causation.as_ref() == Some(&request.causation)
        && facts.workflow_permission_causation.as_ref() == Some(&request.causation)
        && facts
            .workflow_attempt_capability_digest
            .is_some_and(|digest| request.capability.matches_digest(digest));
    if valid {
        Ok(())
    } else {
        Err(map_err(SecretError::PermissionDenied))
    }
}

fn verify_caller(agent: &AgentPrincipal, caller: &SecretCaller) -> Result<(), SecretError> {
    if caller.agent_name != agent.execution.address.agent || caller.depth != agent.depth {
        return Err(SecretError::PermissionDenied);
    }
    Ok(())
}

fn audit_ctx(agent: &AgentPrincipal, caller: &SecretCaller) -> AuditContext {
    AuditContext {
        session_id: agent.session_id.clone(),
        agent_name: caller.agent_name.clone(),
        depth: caller.depth,
        tool_name: caller.tool_name.clone(),
    }
}

fn map_err(error: SecretError) -> String {
    let error = match error {
        SecretError::SecretNotFound(name) => SecretIpcError::SecretNotFound { name },
        SecretError::VaultNotFound(path) => SecretIpcError::VaultNotFound {
            cwd: path.display().to_string(),
        },
        SecretError::PermissionDenied => SecretIpcError::PermissionDenied,
        SecretError::DecryptFailed(detail) => SecretIpcError::DecryptFailed { detail },
        SecretError::InvalidName(name) => SecretIpcError::InvalidName { name },
        SecretError::TemplateParse(detail) => SecretIpcError::TemplateParse { detail },
        SecretError::Ipc(detail) => SecretIpcError::Ipc { detail },
    };
    serde_json::to_string(&error).unwrap_or_else(|e| format!("ipc_encode_failed: {e}"))
}

#[cfg(test)]
#[path = "secret_handler_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "secret_handler_success_tests.rs"]
mod success_tests;

#[cfg(test)]
#[path = "secret_provider_handler_tests.rs"]
mod provider_tests;
