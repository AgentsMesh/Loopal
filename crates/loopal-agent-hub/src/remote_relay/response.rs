use std::sync::Arc;

use loopal_protocol::UserQuestionResponse;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::pending_relay;
use crate::{Hub, HubUplink};

#[path = "response/claim.rs"]
mod claim;
use claim::{ForwardingClaim, claim_record};

pub(super) async fn cancel_question(
    hub: &Arc<Mutex<Hub>>,
    params: &Value,
    active_uplink: &Arc<HubUplink>,
) -> Result<Value, String> {
    let origin_hub = super::required(params, "origin_hub")?;
    let origin_agent = super::required(params, "agent_name")?;
    let interaction_id = super::required(params, "interaction_id")?;
    let key = (
        format!("{origin_hub}/{origin_agent}"),
        interaction_id.to_string(),
    );
    let record = {
        let mut h = hub.lock().await;
        if !h
            .uplink
            .as_ref()
            .is_some_and(|uplink| Arc::ptr_eq(uplink, active_uplink))
        {
            return Err("remote cancellation arrived on a stale uplink generation".into());
        }
        h.pending_remote_questions
            .get(&key)
            .is_some_and(|record| Arc::ptr_eq(&record.uplink, active_uplink))
            .then(|| h.pending_remote_questions.remove(&key))
            .flatten()
    };
    let Some(record) = record else {
        return Ok(json!({"resolved": false}));
    };
    pending_relay::deliver_terminal_event(hub, super::remote_resolved_event(&record)).await?;
    Ok(json!({"resolved": true}))
}

pub(super) async fn resolve_origin_question(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    active_uplink: &Arc<HubUplink>,
) -> Result<Value, String> {
    let agent_name = super::required(&params, "agent_name")?;
    let interaction_id = super::required(&params, "question_id")?;
    let response: UserQuestionResponse = serde_json::from_value(
        params
            .get("response")
            .cloned()
            .ok_or_else(|| "missing response field".to_string())?,
    )
    .map_err(|error| format!("bad response: {error}"))?;
    let resolved = pending_relay::resolve_remote_question(
        hub,
        agent_name,
        interaction_id,
        response,
        active_uplink,
    )
    .await?;
    Ok(json!({"resolved": resolved}))
}

pub(super) async fn forward_question_response(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    mut payload: Value,
) -> Result<bool, String> {
    let interaction_id = super::required(&payload, "question_id")?.to_string();
    let response: UserQuestionResponse = serde_json::from_value(
        payload
            .get("response")
            .cloned()
            .ok_or_else(|| "missing response field".to_string())?,
    )
    .map_err(|error| format!("bad response: {error}"))?;
    if response.question_id() != interaction_id {
        return Err(format!(
            "question response id mismatch: outer '{interaction_id}', body '{}'",
            response.question_id()
        ));
    }
    let key = (agent_name.to_string(), interaction_id);
    let record = claim_record(hub, &key).await?;
    let Some(record) = record else {
        return Ok(false);
    };
    let mut claim = ForwardingClaim::new(hub.clone(), key.clone(), record.uplink.clone());
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "agent_name".into(),
            Value::String(record.origin_agent.clone()),
        );
    }
    let relay = record
        .uplink
        .relay_remote(json!({
            "target_hub": record.origin_hub,
            "operation": "question_response",
            "payload": payload,
        }))
        .await;
    let relay = match relay {
        Ok(value) => value,
        Err(error) => {
            claim.release().await;
            return Err(error);
        }
    };
    match relay.get("resolved").and_then(Value::as_bool) {
        Some(true) => {}
        Some(false) => {
            claim.release().await;
            return Ok(false);
        }
        None => {
            claim.release().await;
            return Err("remote question response missing resolved acknowledgement".into());
        }
    }
    let removed = {
        let mut h = hub.lock().await;
        h.pending_remote_questions
            .get(&key)
            .is_some_and(|current| Arc::ptr_eq(&current.uplink, &record.uplink))
            .then(|| h.pending_remote_questions.remove(&key))
            .flatten()
    };
    claim.commit();
    if let Some(record) = removed {
        pending_relay::deliver_terminal_event(hub, super::remote_resolved_event(&record)).await?;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "response/tests.rs"]
mod tests;
