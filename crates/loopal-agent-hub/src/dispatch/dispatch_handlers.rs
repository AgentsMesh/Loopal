//! Hub request handlers — `hub/*` method implementations.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::Hub;
use crate::routing;

#[path = "agent_command.rs"]
mod agent_command;

pub async fn handle_route(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let mut envelope: Envelope =
        serde_json::from_value(params).map_err(|e| format!("invalid envelope: {e}"))?;
    let cwd = {
        let h = hub.lock().await;
        h.spawn_registry
            .cwd_of(&envelope.target.agent)
            .unwrap_or_else(|| h.default_cwd.clone())
    };
    super::skill_routing::expand_human_skill(&mut envelope, &cwd);

    // Remote address → uplink immediately (target carries the next hop).
    if envelope.target.is_remote() {
        return route_via_uplink(hub, &envelope).await;
    }

    // Local lookup
    let result = {
        let h = hub.lock().await;
        h.registry
            .get_agent_connection(&envelope.target.agent)
            .map(|conn| {
                let observation = routing::RouteObservation::from_hub(&h, &envelope.target.agent);
                (conn, observation)
            })
    };

    match result {
        Some((conn, observation)) => {
            routing::route_to_agent(&conn, &envelope, &observation).await?;
            Ok(json!({"ok": true}))
        }
        None => {
            // Local miss — escalate to MetaHub if uplink exists
            route_via_uplink(hub, &envelope).await
        }
    }
}

/// Forward an envelope to MetaHub via uplink. Errors if no uplink.
async fn route_via_uplink(hub: &Arc<Mutex<Hub>>, envelope: &Envelope) -> Result<Value, String> {
    let uplink = {
        let h = hub.lock().await;
        h.uplink.clone()
    };
    match uplink {
        Some(ul) => {
            ul.route(envelope).await?;
            Ok(json!({"ok": true}))
        }
        None => Err(format!(
            "agent '{}' not found locally and no MetaHub uplink configured",
            envelope.target
        )),
    }
}

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
        h.registry
            .get_agent_connection(&target)
            .ok_or_else(|| format!("no agent: '{target}'"))?
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
