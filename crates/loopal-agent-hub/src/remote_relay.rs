use std::sync::Arc;

use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, ResolveSource};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::dispatch::dispatch_handlers;
use crate::pending_relay::PendingRemoteQuestionInfo;
use crate::request_principal::TrustedMetaHubPrincipal;
use crate::{Hub, HubUplink};

mod admission;
mod cleanup;
mod response;
#[cfg(test)]
#[path = "remote_relay_tests.rs"]
mod tests;

pub(crate) use cleanup::{
    cancel_destination_detached, cancel_remote_origins, resolve_remote_records,
};

pub(crate) async fn handle(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    trusted: &TrustedMetaHubPrincipal,
) -> Result<Value, String> {
    let active_uplink = {
        let h = hub.lock().await;
        h.uplink
            .clone()
            .filter(|uplink| trusted.matches_connection(uplink.connection()))
            .ok_or_else(|| "remote relay arrived on a stale uplink generation".to_string())?
    };
    match params["operation"].as_str().unwrap_or("") {
        "question_request" => admission::emit_question(hub, &params, active_uplink).await,
        "question_response" => {
            response::resolve_origin_question(hub, params["payload"].clone(), &active_uplink).await
        }
        "question_cancel" => response::cancel_question(hub, &params, &active_uplink).await,
        "control" => dispatch_handlers::handle_control(hub, params["payload"].clone()).await,
        "interrupt" => dispatch_handlers::handle_interrupt(hub, params["payload"].clone()).await,
        operation => Err(format!("unsupported remote relay operation: {operation}")),
    }
}

pub(crate) async fn forward_action(
    hub: &Arc<Mutex<Hub>>,
    target: &str,
    operation: &str,
    mut payload: Value,
) -> Result<Value, String> {
    let Some((uplink, next_hub, remaining)) = route(hub, target).await else {
        return Err(format!("remote target required: {target}"));
    };
    if let Some(object) = payload.as_object_mut() {
        object.insert("target".into(), Value::String(remaining));
    }
    uplink
        .relay_remote(json!({
            "target_hub": next_hub, "operation": operation, "payload": payload,
        }))
        .await
}

pub(crate) async fn forward_question_response(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    payload: Value,
) -> Result<bool, String> {
    response::forward_question_response(hub, agent_name, payload).await
}

fn remote_resolved_event(record: &PendingRemoteQuestionInfo) -> AgentEvent {
    AgentEvent::named(
        QualifiedAddress::local(&record.qualified_agent),
        AgentEventPayload::UserQuestionResolved {
            id: record.interaction_id.clone(),
            by: ResolveSource::Manual,
        },
    )
}

async fn route(hub: &Arc<Mutex<Hub>>, target: &str) -> Option<(Arc<HubUplink>, String, String)> {
    let mut address = QualifiedAddress::parse(target);
    let next_hub = address.pop_front_hub()?;
    let uplink = hub.lock().await.uplink.clone()?;
    Some((uplink, next_hub, address.to_string()))
}

fn required<'a>(params: &'a Value, key: &str) -> Result<&'a str, String> {
    params[key]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("remote relay missing {key}"))
}
