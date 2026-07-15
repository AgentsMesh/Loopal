use std::sync::Arc;

use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::dispatch::{dispatch_handlers, relay_response_handlers};
use crate::{Hub, HubUplink};

pub(crate) async fn handle(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    from: &str,
) -> Result<Value, String> {
    if !from.starts_with("meta:") {
        return Err("remote relay requires a MetaHub connection".into());
    }
    match params["operation"].as_str().unwrap_or("") {
        "question_request" => emit_question(hub, &params).await,
        "question_response" => {
            relay_response_handlers::handle_question_response(hub, params["payload"].clone()).await
        }
        "control" => dispatch_handlers::handle_control(hub, params["payload"].clone()).await,
        "interrupt" => dispatch_handlers::handle_interrupt(hub, params["payload"].clone()).await,
        operation => Err(format!("unsupported remote relay operation: {operation}")),
    }
}

async fn emit_question(hub: &Arc<Mutex<Hub>>, params: &Value) -> Result<Value, String> {
    let origin_hub = required(params, "origin_hub")?;
    let agent = required(params, "agent_name")?;
    let payload: AgentEventPayload = serde_json::from_value(params["payload"].clone())
        .map_err(|error| format!("invalid remote question payload: {error}"))?;
    if !matches!(payload, AgentEventPayload::UserQuestionRequest { .. }) {
        return Err("remote question relay received a non-question payload".into());
    }
    let event = AgentEvent::named(
        QualifiedAddress::local(format!("{origin_hub}/{agent}")),
        payload,
    );
    let emitted = hub
        .lock()
        .await
        .registry
        .event_sender()
        .try_send(event)
        .is_ok();
    Ok(json!({"emitted": emitted}))
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
    mut payload: Value,
) -> Result<bool, String> {
    let Some((uplink, next_hub, remaining)) = route(hub, agent_name).await else {
        return Ok(false);
    };
    if let Some(object) = payload.as_object_mut() {
        object.insert("agent_name".into(), Value::String(remaining));
    }
    uplink
        .relay_remote(json!({
            "target_hub": next_hub,
            "operation": "question_response",
            "payload": payload,
        }))
        .await?;
    Ok(true)
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
