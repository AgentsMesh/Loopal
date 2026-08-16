use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_ipc::{DispatcherBuilder, RpcError};
use loopal_protocol::WorkflowWorkerHandshakeRequest;
use tokio::sync::Mutex;

use crate::Hub;
use crate::request_principal::AgentPrincipal;
use crate::types::{AgentOrigin, AgentRuntimeFacts};
use crate::workflow::{WorkflowCoordinatorHandle, WorkflowOwner, owner_for_managed_root};

use super::{decode, encode, string_err_to_rpc};

pub(super) fn register(builder: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let target = hub.clone();
    let builder = builder.register_fn(methods::HUB_WORKFLOW_START.name, move |params, ctx| {
        let target = target.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let (owner, coordinator) = authority(&target, &agent?).await?;
            let request = decode(params)?;
            encode(
                coordinator
                    .start(owner, request)
                    .await
                    .map_err(coordinator_error)?,
            )
        })
    });
    let target = hub.clone();
    let builder = builder.register_fn(
        methods::HUB_WORKFLOW_LOOKUP_START.name,
        move |params, ctx| {
            let target = target.clone();
            let agent = crate::dispatch::authorization::agent(ctx);
            Box::pin(async move {
                let (owner, coordinator) = authority(&target, &agent?).await?;
                let request = decode(params)?;
                encode(
                    coordinator
                        .lookup_start(owner, request)
                        .await
                        .map_err(coordinator_error)?,
                )
            })
        },
    );
    let target = hub.clone();
    let builder = builder.register_fn(methods::HUB_WORKFLOW_GET.name, move |params, ctx| {
        let target = target.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let (owner, coordinator) = authority(&target, &agent?).await?;
            let request = decode(params)?;
            encode(
                coordinator
                    .get(owner, request)
                    .await
                    .map_err(coordinator_error)?,
            )
        })
    });
    let target = hub.clone();
    let builder = builder.register_fn(methods::HUB_WORKFLOW_WAIT.name, move |params, ctx| {
        let target = target.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let (owner, coordinator) = authority(&target, &agent?).await?;
            let request = decode(params)?;
            encode(
                coordinator
                    .wait(owner, request)
                    .await
                    .map_err(coordinator_error)?,
            )
        })
    });
    let target = hub.clone();
    let builder = builder.register_fn(
        methods::HUB_WORKFLOW_WORKER_HANDSHAKE.name,
        move |params, ctx| {
            let target = target.clone();
            let agent = crate::dispatch::authorization::agent(ctx);
            Box::pin(async move {
                let agent = agent?;
                let request: WorkflowWorkerHandshakeRequest = decode(params)?;
                let (owner, coordinator) = worker_authority(&target, &agent, &request).await?;
                encode(
                    coordinator
                        .worker_handshake(
                            owner,
                            crate::workflow::recovery::WorkflowAttemptReconnect {
                                causation: request.causation,
                                capability: request.capability,
                                execution: agent.execution,
                            },
                        )
                        .await
                        .map_err(coordinator_error)?,
                )
            })
        },
    );
    builder.register_fn(methods::HUB_WORKFLOW_CANCEL.name, move |params, ctx| {
        let target = hub.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let (owner, coordinator) = authority(&target, &agent?).await?;
            let request = decode(params)?;
            encode(
                coordinator
                    .cancel(owner, request)
                    .await
                    .map_err(coordinator_error)?,
            )
        })
    })
}

async fn worker_authority(
    hub: &Arc<Mutex<Hub>>,
    principal: &AgentPrincipal,
    request: &WorkflowWorkerHandshakeRequest,
) -> Result<(WorkflowOwner, WorkflowCoordinatorHandle), RpcError> {
    let locked = hub.lock().await;
    if !locked.registry.owns_active_lease(&principal.execution) {
        return Err(string_err_to_rpc("stale Agent connection".into()));
    }
    let facts = locked
        .registry
        .runtime_facts(&principal.execution)
        .ok_or_else(|| string_err_to_rpc("missing workflow worker runtime authority".into()))?;
    if facts.origin != AgentOrigin::ManagedChild
        || facts.depth == 0
        || facts.parent.is_none()
        || facts.root.is_empty()
        || facts.workflow_permission_causation.as_ref() != Some(&request.causation)
        || facts
            .workflow_attempt_capability_digest
            .is_none_or(|digest| digest != request.capability.digest())
    {
        return Err(string_err_to_rpc(
            "workflow worker handshake authority is invalid".into(),
        ));
    }
    let parent = facts
        .parent
        .as_ref()
        .ok_or_else(|| string_err_to_rpc("workflow worker parent lease is missing".into()))?;
    let parent_facts = locked
        .registry
        .runtime_facts(parent)
        .ok_or_else(|| string_err_to_rpc("workflow worker parent authority is stale".into()))?;
    let owner = owner_for_managed_root(parent, parent_facts)
        .map_err(|message| string_err_to_rpc(message.into()))?;
    let coordinator = locked
        .workflow_coordinator()
        .ok_or_else(|| string_err_to_rpc("workflow execution backend is unavailable".into()))?;
    Ok((owner, coordinator))
}

async fn authority(
    hub: &Arc<Mutex<Hub>>,
    principal: &AgentPrincipal,
) -> Result<(WorkflowOwner, WorkflowCoordinatorHandle), RpcError> {
    let locked = hub.lock().await;
    let facts = locked
        .registry
        .runtime_facts(&principal.execution)
        .filter(|_| locked.registry.owns_active_lease(&principal.execution))
        .ok_or_else(|| string_err_to_rpc("stale Agent connection".into()))?;
    let owner = owner(principal, facts)?;
    let coordinator = locked
        .workflow_coordinator()
        .ok_or_else(|| string_err_to_rpc("workflow execution backend is unavailable".into()))?;
    Ok((owner, coordinator))
}

fn owner(principal: &AgentPrincipal, facts: &AgentRuntimeFacts) -> Result<WorkflowOwner, RpcError> {
    owner_for_managed_root(&principal.execution, facts)
        .map_err(|message| string_err_to_rpc(message.into()))
}

fn coordinator_error(error: crate::workflow::WorkflowCoordinatorError) -> RpcError {
    string_err_to_rpc(error.to_string())
}

#[cfg(test)]
#[path = "workflow_tests/mod.rs"]
mod tests;
