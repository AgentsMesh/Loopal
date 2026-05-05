use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::Hub;
use crate::pending_relay;

pub async fn handle_permission_response(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
) -> Result<Value, String> {
    let agent_name = params
        .get("agent_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing agent_name".to_string())?
        .to_string();
    let tool_call_id = params
        .get("tool_call_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing tool_call_id".to_string())?
        .to_string();
    let allow = params
        .get("allow")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| "missing allow".to_string())?;
    let resolved = pending_relay::resolve_permission(hub, &agent_name, &tool_call_id, allow).await;
    Ok(json!({"resolved": resolved}))
}

pub async fn handle_question_response(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
) -> Result<Value, String> {
    let agent_name = params
        .get("agent_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing agent_name".to_string())?
        .to_string();
    let question_id = params
        .get("question_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing or empty question_id".to_string())?
        .to_string();
    let response_value = params
        .get("response")
        .ok_or_else(|| "missing response field".to_string())?
        .clone();
    let response: loopal_protocol::UserQuestionResponse =
        serde_json::from_value(response_value).map_err(|e| format!("bad response: {e}"))?;
    let resolved = pending_relay::resolve_question(hub, &agent_name, &question_id, response).await;
    Ok(json!({"resolved": resolved}))
}
