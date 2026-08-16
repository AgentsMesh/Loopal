//! Hub request handlers — `hub/*` method implementations.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::protocol::methods;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::Hub;

pub(super) use super::route_handler::handle_route;

#[path = "agent_command.rs"]
mod agent_command;

pub async fn handle_list_agents(hub: &Arc<Mutex<Hub>>) -> Result<Value, String> {
    let agents: Vec<Value> = hub
        .lock()
        .await
        .registry
        .list_agents()
        .into_iter()
        .map(|(name, state)| json!({"name": name, "state": state}))
        .collect();
    Ok(json!({"agents": agents}))
}

pub async fn handle_control(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let target = params["target"]
        .as_str()
        .ok_or("missing 'target' field")?
        .to_string();
    if loopal_protocol::QualifiedAddress::parse(&target).is_remote() {
        let response = crate::remote_relay::forward_action(hub, &target, "control", params).await?;
        return agent_command::normalize_control_response(response)
            .map_err(|error| format!("control to '{target}' returned {error}"));
    }
    let command = params["command"].clone();
    let conn = {
        let h = hub.lock().await;
        let conn = h
            .registry
            .get_agent_connection(&target)
            .ok_or_else(|| format!("no agent: '{target}'"))?;
        let execution = h.registry.execution_for_connection(&target, &conn);
        if h.workflow_coordinator().is_some()
            && serde_json::from_value::<loopal_protocol::ControlCommand>(command.clone()).is_ok_and(
                |command| matches!(command, loopal_protocol::ControlCommand::ResumeSession(_)),
            )
            && execution
                .as_ref()
                .and_then(|execution| h.registry.runtime_facts(execution))
                .is_some_and(|facts| {
                    facts.origin == crate::types::AgentOrigin::ManagedRoot
                        && facts.parent.is_none()
                        && facts.depth == 0
                        && facts.root == loopal_protocol::ROOT_AGENT_NAME
                })
        {
            return Err(
                "root session hot-swap is unavailable while workflow execution is enabled".into(),
            );
        }
        conn
    };
    let response = agent_command::request(
        Arc::clone(&conn),
        methods::AGENT_CONTROL.name,
        command,
        &target,
        "control",
        agent_command::CONTROL_DEADLINE,
        agent_command::TimeoutDisposition::PreserveAsUnknown,
    )
    .await?;
    match agent_command::normalize_control_response(response) {
        Ok(response) => Ok(response),
        Err(error) => {
            let _ = tokio::time::timeout(Duration::from_secs(2), conn.close()).await;
            Err(format!("control to '{target}' returned {error}"))
        }
    }
}

pub async fn handle_interrupt(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let target = params["target"]
        .as_str()
        .ok_or("missing 'target' field")?
        .to_string();
    if loopal_protocol::QualifiedAddress::parse(&target).is_remote() {
        return crate::remote_relay::forward_action(hub, &target, "interrupt", params).await;
    }
    tracing::info!(target, "handle_interrupt: looking up agent connection");
    let conn = {
        let h = hub.lock().await;
        h.registry
            .get_agent_connection(&target)
            .ok_or_else(|| format!("no agent: '{target}'"))?
    };
    let hub = hub.clone();
    let coordinator = tokio::spawn(async move {
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            conn.send_request(methods::AGENT_INTERRUPT.name, json!({})),
        )
        .await;
        let error = match result {
            Ok(Ok(_)) => None,
            Ok(Err(error)) => Some(format!("interrupt to '{target}' failed: {error}")),
            Err(_) => Some(format!("interrupt to '{target}' timed out")),
        };
        if error.is_some() {
            let _ = tokio::time::timeout(Duration::from_secs(2), conn.close()).await;
        }
        crate::pending_relay::cancel_pending_for_agent_connection(&hub, &target, &conn).await;
        match error {
            Some(error) => Err(error),
            None => {
                tracing::info!(target, "handle_interrupt: interrupt acknowledged");
                Ok(json!({"ok": true}))
            }
        }
    });
    coordinator
        .await
        .map_err(|error| format!("interrupt coordinator failed: {error}"))?
}

pub async fn handle_shutdown_agent(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let target = params["target"]
        .as_str()
        .ok_or("missing 'target' field")?
        .to_string();
    let conn = {
        let h = hub.lock().await;
        h.registry
            .get_agent_connection(&target)
            .ok_or_else(|| format!("no agent: '{target}'"))?
    };
    agent_command::request(
        conn,
        methods::AGENT_SHUTDOWN.name,
        json!({}),
        &target,
        "shutdown",
        agent_command::SHUTDOWN_DEADLINE,
        agent_command::TimeoutDisposition::CloseConnection,
    )
    .await?;
    Ok(json!({"ok": true}))
}

#[cfg(test)]
#[path = "interrupt_generation_tests.rs"]
mod interrupt_generation_tests;

#[cfg(test)]
#[path = "interrupt_tests.rs"]
mod interrupt_tests;

#[cfg(test)]
#[path = "workflow_control_tests.rs"]
mod workflow_control_tests;
