use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;

use crate::dispatch::dispatch_hub_request_with_principal;
use crate::hub::Hub;
use crate::pending_relay::{
    handle_agent_permission, handle_agent_plan_approval, handle_agent_question,
};
use crate::request_principal::{AgentPrincipal, HubRequestPrincipal};
use crate::types::AgentExecutionRef;

const WAIT_AGENT_METHOD: &str = "hub/wait_agent";
const WORKFLOW_WAIT_METHOD: &str = "hub/workflow/wait";

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_request(
    hub: &Arc<Mutex<Hub>>,
    dispatcher: &Arc<loopal_ipc::Dispatcher>,
    connection: &Arc<Connection<Listening>>,
    agent_name: &str,
    execution: &AgentExecutionRef,
    request_id: i64,
    method: String,
    params: serde_json::Value,
) {
    if method.starts_with("hub/") || method.starts_with("meta/") {
        let principal = match agent_principal(hub, execution).await {
            Ok(principal) => principal,
            Err(error) => {
                respond_error(connection, request_id, agent_name, &method, error).await;
                return;
            }
        };
        if method == WAIT_AGENT_METHOD || method == WORKFLOW_WAIT_METHOD {
            spawn_background_wait(
                hub.clone(),
                dispatcher.clone(),
                connection.clone(),
                request_id,
                params,
                agent_name.to_string(),
                principal,
                method,
            );
            return;
        }
        info!(agent = %agent_name, %method, "hub request received");
        match dispatch_hub_request_with_principal(hub, dispatcher, &method, params, principal).await
        {
            Ok(result) => {
                let _ = connection.respond(request_id, result).await;
            }
            Err(error) => {
                respond_error(connection, request_id, agent_name, &method, error).await;
            }
        }
        info!(agent = %agent_name, %method, "hub request completed");
        return;
    }
    let interactive = method == methods::AGENT_PERMISSION.name
        || method == methods::AGENT_QUESTION.name
        || method == methods::AGENT_PLAN_APPROVAL.name;
    if interactive && !active_interaction_lease(hub, execution, connection).await {
        respond_error(
            connection,
            request_id,
            agent_name,
            &method,
            "stale Agent connection lease".into(),
        )
        .await;
        return;
    }
    if method == methods::AGENT_PERMISSION.name {
        handle_agent_permission(
            hub,
            connection.clone(),
            request_id,
            params,
            agent_name,
            execution.clone(),
        )
        .await;
    } else if method == methods::AGENT_QUESTION.name {
        handle_agent_question(hub, connection.clone(), request_id, params, agent_name).await;
    } else if method == methods::AGENT_PLAN_APPROVAL.name {
        handle_agent_plan_approval(hub, connection.clone(), request_id, params, agent_name).await;
    } else {
        warn!(agent = %agent_name, %method, "unknown request");
        let _ = connection
            .respond_error(
                request_id,
                loopal_ipc::jsonrpc::METHOD_NOT_FOUND,
                &format!("unknown: {method}"),
            )
            .await;
    }
}

async fn active_interaction_lease(
    hub: &Arc<Mutex<Hub>>,
    execution: &AgentExecutionRef,
    connection: &Arc<Connection<Listening>>,
) -> bool {
    let hub = hub.lock().await;
    hub.registry.owns_active_lease(execution)
        && hub
            .registry
            .exact_connection(execution)
            .is_some_and(|active| Arc::ptr_eq(&active, connection))
}

async fn agent_principal(
    hub: &Arc<Mutex<Hub>>,
    execution: &AgentExecutionRef,
) -> Result<Arc<HubRequestPrincipal>, String> {
    let facts = hub
        .lock()
        .await
        .registry
        .runtime_facts(execution)
        .cloned()
        .ok_or_else(|| "stale Agent connection or missing runtime authority".to_string())?;
    Ok(Arc::new(HubRequestPrincipal::Agent(AgentPrincipal::new(
        execution.clone(),
        facts,
    ))))
}

#[allow(clippy::too_many_arguments)]
fn spawn_background_wait(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    connection: Arc<Connection<Listening>>,
    request_id: i64,
    params: serde_json::Value,
    agent_name: String,
    principal: Arc<HubRequestPrincipal>,
    method: String,
) {
    tokio::spawn(async move {
        let result = dispatch_hub_request_with_principal(
            &hub,
            dispatcher.as_ref(),
            &method,
            params,
            principal,
        )
        .await;
        match result {
            Ok(value) => {
                let _ = connection.respond(request_id, value).await;
            }
            Err(error) => {
                respond_error(&connection, request_id, &agent_name, &method, error).await;
            }
        }
        info!(agent = %agent_name, %method, "background Hub wait resolved");
    });
}

async fn respond_error(
    connection: &Connection<Listening>,
    request_id: i64,
    agent_name: &str,
    method: &str,
    error: String,
) {
    warn!(agent = %agent_name, %method, %error, "hub request failed");
    let _ = connection
        .respond_error(request_id, loopal_ipc::jsonrpc::INVALID_REQUEST, &error)
        .await;
}
