use loopal_ipc::connection::Connection;
use loopal_ipc::jsonrpc;
use loopal_ipc::protocol::methods;
use serde_json::Value;

use crate::server_init::build_initialize_result;
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
    // Idempotent re-handshake: clients may retry `initialize` after a transient
    // timeout (e.g. slow child stdin under sandboxed test runners). The canonical
    // first call is consumed by `wait_for_initialize_with_token`; any subsequent
    // call lands here and must succeed — otherwise the retry path explodes with
    // -32601 even though the connection is healthy.
    if method == methods::INITIALIZE.name {
        let result = build_initialize_result();
        return serde_json::to_value(result)
            .map_err(|e| RpcErrorPayload::internal(format!("encode initialize result: {e}")));
    }
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
