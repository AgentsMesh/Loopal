use std::sync::Arc;

use loopal_ipc::{HandlerCtx, RpcError};
use tokio::sync::Mutex;

use super::authorization_policy::{
    external_agent_method, managed_agent_method, managed_meta_method, root_agent_method,
    trusted_meta_method, workflow_worker_method,
};
use crate::Hub;
use crate::request_principal::{
    AgentPrincipal, HubRequestPrincipal, TrustedMetaHubPrincipal, UiPrincipal,
};

pub(crate) async fn authorize(
    hub: &Arc<Mutex<Hub>>,
    method: &str,
    principal: Arc<HubRequestPrincipal>,
) -> Result<HandlerCtx, RpcError> {
    let from = principal_name(&principal);
    let allowed = match principal.as_ref() {
        HubRequestPrincipal::Ui(ui) => authorize_ui(hub, method, ui).await,
        HubRequestPrincipal::Agent(agent) => authorize_agent(hub, method, agent).await,
        HubRequestPrincipal::TrustedMetaHub(meta) => {
            authorize_trusted_meta(hub, method, meta).await
        }
        HubRequestPrincipal::Internal => true,
    };
    if !allowed {
        return Err(super::make_invalid_request(format!(
            "{method} is not authorized for {from}"
        )));
    }
    Ok(HandlerCtx::new(from).with_extension(principal))
}

pub(crate) fn principal(ctx: &HandlerCtx) -> Result<Arc<HubRequestPrincipal>, RpcError> {
    ctx.extension::<HubRequestPrincipal>()
        .ok_or_else(|| super::make_invalid_request("missing Hub request principal".into()))
}

pub(crate) fn ui(ctx: &HandlerCtx) -> Result<UiPrincipal, RpcError> {
    match principal(ctx)?.as_ref() {
        HubRequestPrincipal::Ui(ui) => Ok(ui.clone()),
        _ => Err(super::make_invalid_request(
            "method requires a UI principal".into(),
        )),
    }
}

pub(crate) fn agent(ctx: &HandlerCtx) -> Result<AgentPrincipal, RpcError> {
    match principal(ctx)?.as_ref() {
        HubRequestPrincipal::Agent(agent) => Ok(agent.clone()),
        _ => Err(super::make_invalid_request(
            "method requires an Agent principal".into(),
        )),
    }
}

pub(crate) fn trusted_meta(ctx: &HandlerCtx) -> Result<TrustedMetaHubPrincipal, RpcError> {
    match principal(ctx)?.as_ref() {
        HubRequestPrincipal::TrustedMetaHub(meta) => Ok(meta.clone()),
        _ => Err(super::make_invalid_request(
            "method requires an authenticated reverse MetaHub transport".into(),
        )),
    }
}

pub(crate) async fn revalidate_agent(
    hub: &Arc<Mutex<Hub>>,
    agent: &AgentPrincipal,
) -> Result<(), RpcError> {
    if hub
        .lock()
        .await
        .registry
        .owns_active_lease(&agent.execution)
    {
        Ok(())
    } else {
        Err(super::make_invalid_request("stale Agent connection".into()))
    }
}

async fn authorize_ui(hub: &Arc<Mutex<Hub>>, method: &str, ui: &UiPrincipal) -> bool {
    let Some((name, capabilities, connection)) = hub.lock().await.ui.client_lease(&ui.lease_id)
    else {
        return false;
    };
    name == ui.name
        && capabilities == ui.capabilities
        && ui.matches_connection(&connection)
        && crate::ui_request_policy::is_ui_request(method)
}

async fn authorize_agent(hub: &Arc<Mutex<Hub>>, method: &str, agent: &AgentPrincipal) -> bool {
    let locked = hub.lock().await;
    if !locked.registry.owns_active_lease(&agent.execution) {
        return false;
    }
    if !agent.is_managed() {
        return !method.starts_with("meta/") && external_agent_method(method);
    }
    if agent.workflow_permission_causation.is_some() {
        return workflow_worker_method(method);
    }
    if method.starts_with("meta/") {
        return managed_meta_method(method);
    }
    managed_agent_method(method)
        || (matches!(agent.origin, crate::types::AgentOrigin::ManagedRoot)
            && agent.depth == 0
            && root_agent_method(method, locked.workflow_coordinator().is_some()))
}

async fn authorize_trusted_meta(
    hub: &Arc<Mutex<Hub>>,
    method: &str,
    meta: &TrustedMetaHubPrincipal,
) -> bool {
    trusted_meta_method(method)
        && hub
            .lock()
            .await
            .uplink
            .as_ref()
            .is_some_and(|uplink| meta.matches_connection(uplink.connection()))
}

fn principal_name(principal: &HubRequestPrincipal) -> String {
    match principal {
        HubRequestPrincipal::Ui(ui) => ui.lease_id.clone(),
        HubRequestPrincipal::Agent(agent) => agent.address().agent.clone(),
        HubRequestPrincipal::TrustedMetaHub(_) => "trusted-metahub".into(),
        HubRequestPrincipal::Internal => "internal".into(),
    }
}
