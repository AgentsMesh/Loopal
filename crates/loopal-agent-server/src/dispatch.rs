use loopal_ipc::connection::Connection;
use loopal_ipc::jsonrpc;
use loopal_ipc::protocol::methods;
use serde_json::Value;

use crate::session_hub::SessionHub;

#[derive(Debug)]
pub struct RpcErrorPayload {
    pub code: i64,
    pub message: String,
}

impl RpcErrorPayload {
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: jsonrpc::INTERNAL_ERROR,
            message: message.into(),
        }
    }

    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self {
            code: jsonrpc::METHOD_NOT_FOUND,
            message: message.into(),
        }
    }
}

pub async fn dispatch_simple(method: &str, hub: &SessionHub) -> Result<Value, RpcErrorPayload> {
    if method == methods::AGENT_SHUTDOWN.name {
        return Ok(serde_json::json!({"ok": true}));
    }
    if method == methods::AGENT_LIST.name {
        let ids = hub.list_session_ids().await;
        let sessions: Vec<_> = ids
            .iter()
            .map(|id| serde_json::json!({"session_id": id}))
            .collect();
        return Ok(serde_json::json!(sessions));
    }
    Err(RpcErrorPayload::method_not_found(format!(
        "unexpected method: {method}"
    )))
}

pub async fn respond_with(
    connection: &Connection,
    id: i64,
    outcome: Result<Value, RpcErrorPayload>,
) {
    let _ = match outcome {
        Ok(v) => connection.respond(id, v).await,
        Err(e) => connection.respond_error(id, e.code, &e.message).await,
    };
}
