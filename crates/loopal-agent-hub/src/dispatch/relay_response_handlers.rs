use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, ResolveSource};

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
    let remember_session = params
        .get("remember_session")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let resolved =
        pending_relay::resolve_permission(hub, &agent_name, &tool_call_id, allow, remember_session)
            .await;
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
    if !resolved
        && crate::remote_relay::forward_question_response(
            hub,
            &agent_name,
            json!({
                "agent_name": agent_name.clone(),
                "question_id": question_id.clone(),
                "response": params["response"].clone(),
            }),
        )
        .await?
    {
        emit_remote_question_resolved(hub, &agent_name, &question_id).await;
        return Ok(json!({"resolved": true}));
    }
    Ok(json!({"resolved": resolved}))
}

async fn emit_remote_question_resolved(hub: &Arc<Mutex<Hub>>, agent_name: &str, question_id: &str) {
    let event = AgentEvent::named(
        QualifiedAddress::local(agent_name),
        AgentEventPayload::UserQuestionResolved {
            id: question_id.to_string(),
            by: ResolveSource::Manual,
        },
    );
    let _ = hub.lock().await.registry.event_sender().try_send(event);
}

pub async fn handle_plan_approval_response(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
) -> Result<Value, String> {
    let agent_name = required_string(&params, "agent_name")?;
    let request_id = required_string(&params, "request_id")?;
    let decision = required_string(&params, "decision")?;
    let response = match decision.as_str() {
        "approve" | "reject" => json!({"decision": decision}),
        "approve_with_edits" => json!({
            "decision": decision,
            "edited_plan": required_string(&params, "edited_plan")?,
        }),
        _ => return Err("invalid plan approval decision".to_string()),
    };
    let resolved =
        pending_relay::resolve_plan_approval(hub, &agent_name, &request_id, response).await;
    Ok(json!({"resolved": resolved}))
}

fn required_string(params: &Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .map(String::from)
        .ok_or_else(|| format!("missing or empty {key}"))
}
