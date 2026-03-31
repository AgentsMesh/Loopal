//! JSON-RPC 2.0 message types and parsing (newline-delimited).
//!
//! Extracted from `loopal-acp` to serve as the shared JSON-RPC foundation
//! for all IPC communication (consumer↔agent, agent↔sub-agent, IDE↔agent).

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Message types ────────────────────────────────────────────────────

/// A parsed incoming JSON-RPC message.
#[derive(Debug)]
pub enum IncomingMessage {
    Request {
        id: i64,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    Response {
        id: i64,
        result: Option<Value>,
        error: Option<JsonRpcError>,
    },
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ── Standard error codes ─────────────────────────────────────────────

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INTERNAL_ERROR: i64 = -32603;

// ── Raw envelope for deserialization ─────────────────────────────────

#[derive(Deserialize)]
struct RawMessage {
    id: Option<Value>,
    method: Option<String>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
    #[serde(default)]
    params: Value,
}

impl RawMessage {
    fn classify(self) -> Option<IncomingMessage> {
        if let Some(method) = self.method {
            match self.id {
                Some(Value::Number(n)) => {
                    let id = n.as_i64().unwrap_or(0);
                    Some(IncomingMessage::Request {
                        id,
                        method,
                        params: self.params,
                    })
                }
                Some(_) => None, // non-numeric id: skip
                None => Some(IncomingMessage::Notification {
                    method,
                    params: self.params,
                }),
            }
        } else if let Some(Value::Number(n)) = self.id {
            let id = n.as_i64().unwrap_or(0);
            Some(IncomingMessage::Response {
                id,
                result: self.result,
                error: self.error,
            })
        } else {
            None
        }
    }
}

// ── Parsing ──────────────────────────────────────────────────────────

/// Parse a single JSON-RPC message from raw bytes. Returns `None` if malformed.
pub fn parse_message(data: &[u8]) -> Option<IncomingMessage> {
    let raw: RawMessage = serde_json::from_slice(data).ok()?;
    raw.classify()
}

/// Read one JSON-RPC message from a buffered reader. Returns `None` on EOF.
///
/// This is a convenience function for non-Transport usage (e.g. legacy ACP).
/// For Transport-based communication, use `Connection` instead.
pub async fn read_message(
    reader: &mut (impl tokio::io::AsyncBufReadExt + Unpin),
) -> Option<IncomingMessage> {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => return None, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Some(msg) = parse_message(trimmed.as_bytes()) {
                    return Some(msg);
                }
                // Malformed line — skip
            }
            Err(_) => return None,
        }
    }
}

// ── Serialization helpers ────────────────────────────────────────────

/// Build a JSON-RPC request envelope.
pub fn encode_request(id: i64, method: &str, params: Value) -> Vec<u8> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    serde_json::to_vec(&msg).unwrap_or_default()
}

/// Build a JSON-RPC notification envelope (no id).
pub fn encode_notification(method: &str, params: Value) -> Vec<u8> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    });
    serde_json::to_vec(&msg).unwrap_or_default()
}

/// Build a JSON-RPC success response.
pub fn encode_response(id: i64, result: Value) -> Vec<u8> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    });
    serde_json::to_vec(&msg).unwrap_or_default()
}

/// Build a JSON-RPC error response.
pub fn encode_error(id: i64, code: i64, message: &str) -> Vec<u8> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    });
    serde_json::to_vec(&msg).unwrap_or_default()
}
