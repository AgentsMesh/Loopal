use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use serde_json::{Value, json};
use tokio::sync::mpsc::Receiver;

use super::hub_controls::{HarnessVault, PermissionDesk};
use super::hub_security::{audit_reply, permission_allow_reply};

enum HubReply {
    Ok(Value),
    Err(String),
    Hold,
    Unhandled,
}

pub(super) fn spawn_hub_dispatcher(
    conn: Arc<Connection<Listening>>,
    mut raw_rx: Receiver<Incoming>,
    advertise_mcp: bool,
    calls: Arc<Mutex<Vec<Value>>>,
    vault: Arc<HarnessVault>,
    permissions: Arc<PermissionDesk>,
) -> Receiver<Incoming> {
    let (tx, rx) = tokio::sync::mpsc::channel(256);
    tokio::spawn(async move {
        while let Some(incoming) = raw_rx.recv().await {
            match incoming {
                Incoming::Request { id, method, params } => {
                    let reply = permission_reply(&method, &params, &permissions)
                        .or_else(|| {
                            audit_reply(&method, &params).map(|result| match result {
                                Ok(value) => HubReply::Ok(value),
                                Err(message) => HubReply::Err(message),
                            })
                        })
                        .unwrap_or_else(|| {
                            hub_mcp_reply(&method, &params, advertise_mcp, &calls)
                                .map(HubReply::Ok)
                                .unwrap_or_else(|| hub_secret_reply(&method, &params, &vault))
                        });
                    match reply {
                        HubReply::Ok(value) => {
                            let _ = conn.respond(id, value).await;
                        }
                        HubReply::Err(message) => {
                            let _ = conn.respond_error(id, -32000, &message).await;
                        }
                        HubReply::Hold => continue,
                        HubReply::Unhandled => {
                            let _ = conn
                                .respond_error(id, -32601, "not implemented by e2e harness")
                                .await;
                        }
                    }
                }
                note @ Incoming::Notification { .. } => {
                    if tx.send(note).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    rx
}

fn permission_reply(
    method: &str,
    params: &Value,
    permissions: &PermissionDesk,
) -> Option<HubReply> {
    if method == methods::AGENT_PERMISSION.name {
        permissions.asks.lock().unwrap().push(params.clone());
        if permissions.hold.load(Ordering::Acquire) {
            return Some(HubReply::Hold);
        }
        let allow = permissions.allow.load(Ordering::Acquire);
        return Some(if allow {
            match permission_allow_reply(params) {
                Ok(value) => HubReply::Ok(value),
                Err(message) => HubReply::Err(message),
            }
        } else {
            HubReply::Ok(json!({"allow": false}))
        });
    }
    if method == methods::AGENT_QUESTION.name {
        permissions
            .question_asks
            .lock()
            .unwrap()
            .push(params.clone());
        let answers = permissions.question_answers.lock().unwrap().clone();
        let question_id = params["question_id"].as_str().unwrap_or_default();
        return Some(HubReply::Ok(json!({
            "kind": "answered",
            "question_id": question_id,
            "answers": answers,
        })));
    }
    if method == methods::AGENT_PLAN_APPROVAL.name {
        permissions
            .plan_requests
            .lock()
            .unwrap()
            .push(params.clone());
        let decision = permissions.plan_decision.lock().unwrap().clone();
        return Some(HubReply::Ok(json!({"decision": decision})));
    }
    None
}

fn hub_secret_reply(method: &str, params: &Value, vault: &HarnessVault) -> HubReply {
    if method == methods::HUB_SECRET_GET.name {
        vault.gets.lock().unwrap().push(params.clone());
        if vault.failing.load(Ordering::Acquire) {
            return HubReply::Err("e2e vault outage".into());
        }
        let name = params["name"].as_str().unwrap_or_default();
        return match vault.entries.lock().unwrap().get(name) {
            Some(plaintext) => HubReply::Ok(json!({"plaintext": plaintext})),
            None => HubReply::Err(format!("e2e vault has no entry named {name}")),
        };
    }
    if method == methods::HUB_SECRET_LIST_NAMES.name {
        let names: Vec<String> = vault.entries.lock().unwrap().keys().cloned().collect();
        return HubReply::Ok(json!({"names": names}));
    }
    HubReply::Unhandled
}

fn hub_mcp_reply(
    method: &str,
    params: &Value,
    advertise: bool,
    calls: &Mutex<Vec<Value>>,
) -> Option<Value> {
    if method == methods::HUB_MCP_SNAPSHOT.name {
        let servers = if advertise {
            vec![json!({
                "name": "mock", "transport": "stdio", "source": "project",
                "status": "connected", "tool_count": 1,
                "resource_count": 0, "prompt_count": 0, "errors": []
            })]
        } else {
            vec![]
        };
        return Some(json!({"servers": servers}));
    }
    if method == methods::HUB_MCP_LIST_TOOLS.name {
        let tools = if advertise {
            vec![json!({
                "server": "mock", "name": "mcp_echo",
                "description": "Echo back the given text.",
                "input_schema": {
                    "type": "object",
                    "properties": {"text": {"type": "string"}},
                    "required": ["text"]
                }
            })]
        } else {
            vec![]
        };
        return Some(json!({"tools": tools}));
    }
    if method == methods::HUB_MCP_CALL_TOOL.name {
        calls.lock().unwrap().push(params.clone());
        let text = params["args"]["text"].as_str().unwrap_or_default();
        return Some(json!({
            "content": [{"type": "text", "text": format!("mcp_echo: {text}")}],
            "is_error": false
        }));
    }
    None
}
