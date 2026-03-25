//! JSON-RPC 2.0 transport (newline-delimited JSON, `Send + Sync`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufWriter};
use tokio::sync::{Mutex, oneshot};

// ── Types ───────────────────────────────────────────────────────────

/// A parsed incoming JSON-RPC message (request, notification, or response).
#[derive(Debug)]
pub enum IncomingMessage {
    Request {
        id: i64,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        #[allow(dead_code)]
        params: Value,
    },
    Response {
        id: i64,
        result: Option<Value>,
        error: Option<JsonRpcError>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// Standard JSON-RPC error codes
#[allow(dead_code)]
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INTERNAL_ERROR: i64 = -32603;

/// Raw JSON-RPC envelope used for deserialization.
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
            // Request or notification
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
            // Response to an outbound request
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

// ── Transport ───────────────────────────────────────────────────────

/// Newline-delimited JSON-RPC transport.
pub struct JsonRpcTransport {
    writer: Mutex<BufWriter<Box<dyn AsyncWrite + Unpin + Send>>>,
    pending: Mutex<HashMap<i64, oneshot::Sender<Value>>>,
    next_id: AtomicI64,
}

impl Default for JsonRpcTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonRpcTransport {
    /// Create a transport writing to stdout (production default).
    pub fn new() -> Self {
        Self::with_writer(Box::new(tokio::io::stdout()))
    }

    /// Create a transport writing to an arbitrary async writer.
    ///
    /// Used for testing where stdout is not available.
    pub fn with_writer(writer: Box<dyn AsyncWrite + Unpin + Send>) -> Self {
        Self {
            writer: Mutex::new(BufWriter::new(writer)),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
        }
    }

    /// Send a successful response to a client request.
    pub async fn respond(&self, id: i64, result: Value) {
        let msg = serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result });
        self.write_line(&msg).await;
    }

    /// Send an error response to a client request.
    pub async fn respond_error(&self, id: i64, code: i64, message: &str) {
        let msg = serde_json::json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": code, "message": message }
        });
        self.write_line(&msg).await;
    }

    /// Send a notification (no id) to the client.
    pub async fn notify(&self, method: &str, params: Value) {
        let msg = serde_json::json!({ "jsonrpc": "2.0", "method": method, "params": params });
        self.write_line(&msg).await;
    }

    /// Send a request to the client and wait for the response.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let msg = serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        });
        self.write_line(&msg).await;

        rx.await.map_err(|_| "response channel dropped".to_string())
    }

    /// Route an incoming response to the matching pending request.
    pub async fn route_response(&self, id: i64, value: Value) {
        if let Some(tx) = self.pending.lock().await.remove(&id) {
            let _ = tx.send(value);
        }
    }

    async fn write_line(&self, value: &Value) {
        let mut w = self.writer.lock().await;
        if let Ok(bytes) = serde_json::to_vec(value) {
            let _ = w.write_all(&bytes).await;
            let _ = w.write_all(b"\n").await;
            let _ = w.flush().await;
        }
    }
}

/// Read one JSON-RPC message from a buffered reader. Returns `None` on EOF.
pub async fn read_message(reader: &mut (impl AsyncBufReadExt + Unpin)) -> Option<IncomingMessage> {
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
                if let Ok(raw) = serde_json::from_str::<RawMessage>(trimmed)
                    && let Some(msg) = raw.classify()
                {
                    return Some(msg);
                }
                // Malformed line — skip and continue reading
            }
            Err(_) => return None,
        }
    }
}
