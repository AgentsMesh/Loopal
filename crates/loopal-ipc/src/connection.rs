//! Bidirectional JSON-RPC connection over a `Transport`.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use serde_json::Value;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{debug, warn};

use crate::jsonrpc::{self, IncomingMessage};
use crate::transport::Transport;

/// An incoming message dispatched by the reader loop.
#[derive(Debug)]
pub enum Incoming {
    Request {
        id: i64,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
}

type PendingMap = Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>;

/// Bidirectional JSON-RPC over a `Transport`. Call `start()` first,
/// then use `send_request`, `send_notification`, `respond`, `respond_error`.
pub struct Connection {
    transport: Arc<dyn Transport>,
    pending: PendingMap,
    next_id: AtomicI64,
}

impl Connection {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicI64::new(1),
        }
    }

    /// Spawn the background reader loop. Returns a receiver for incoming
    /// requests and notifications. The loop runs until the transport disconnects.
    pub fn start(&self) -> mpsc::Receiver<Incoming> {
        let (tx, rx) = mpsc::channel::<Incoming>(256);
        let transport = self.transport.clone();
        let pending = self.pending.clone();

        tokio::spawn(async move {
            debug!("IPC reader loop started");
            loop {
                let data = match transport.recv().await {
                    Ok(Some(data)) => data,
                    Ok(None) => {
                        debug!("IPC connection: EOF, reader loop exiting");
                        break;
                    }
                    Err(e) => {
                        warn!("IPC connection read error: {e}");
                        break;
                    }
                };

                let Some(msg) = jsonrpc::parse_message(&data) else {
                    warn!("IPC connection: malformed message, skipping");
                    continue;
                };

                match msg {
                    IncomingMessage::Response { id, result, error } => {
                        let value = if let Some(err) = error {
                            serde_json::to_value(err).unwrap_or(Value::Null)
                        } else {
                            result.unwrap_or(Value::Null)
                        };
                        if let Some(sender) = pending.lock().await.remove(&id) {
                            let _ = sender.send(value);
                        }
                    }
                    IncomingMessage::Request { id, method, params } => {
                        let _ = tx.send(Incoming::Request { id, method, params }).await;
                    }
                    IncomingMessage::Notification { method, params } => {
                        let _ = tx.send(Incoming::Notification { method, params }).await;
                    }
                }
            }

            // Cleanup: drop all pending request senders so callers get Err
            let mut map = pending.lock().await;
            if !map.is_empty() {
                warn!("IPC reader: dropping {} pending requests on exit", map.len());
                map.clear();
            }
        });

        rx
    }

    /// Send a JSON-RPC request and wait for the response.
    ///
    /// Cancellation-safe: if the future is dropped mid-await, the pending
    /// entry is removed from the map to prevent memory leaks.
    pub async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        debug!(id, method, "IPC send_request");
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let data = jsonrpc::encode_request(id, method, params);
        if let Err(e) = self.transport.send(&data).await {
            self.pending.lock().await.remove(&id);
            return Err(format!("transport send failed: {e}"));
        }

        // Guard: remove pending entry if this future is cancelled (dropped)
        let pending = self.pending.clone();
        let guard = PendingGuard {
            id,
            pending: Some(pending),
        };
        let result = rx.await.map_err(|_| "response channel dropped".to_string());
        guard.disarm();
        result
    }

    /// Send a JSON-RPC notification (fire-and-forget, no response expected).
    pub async fn send_notification(&self, method: &str, params: Value) -> Result<(), String> {
        debug!(method, "IPC send_notification");
        let data = jsonrpc::encode_notification(method, params);
        self.transport
            .send(&data)
            .await
            .map_err(|e| format!("transport send failed: {e}"))
    }

    /// Send a successful response to an incoming request.
    pub async fn respond(&self, id: i64, result: Value) -> Result<(), String> {
        debug!(id, "IPC respond ok");
        let data = jsonrpc::encode_response(id, result);
        self.transport
            .send(&data)
            .await
            .map_err(|e| format!("transport send failed: {e}"))
    }

    /// Send an error response to an incoming request.
    pub async fn respond_error(&self, id: i64, code: i64, message: &str) -> Result<(), String> {
        debug!(id, code, message, "IPC respond_error");
        let data = jsonrpc::encode_error(id, code, message);
        self.transport
            .send(&data)
            .await
            .map_err(|e| format!("transport send failed: {e}"))
    }

    /// Check whether the underlying transport is still connected.
    pub fn is_connected(&self) -> bool {
        self.transport.is_connected()
    }
}

// ── Cancellation guard ───────────────────────────────────────────────

/// Removes a pending request entry on drop (cancellation safety).
/// Call `disarm()` on success to skip cleanup. Uses `try_lock` in Drop
/// to avoid spawning async tasks (which is unsafe during runtime shutdown).
struct PendingGuard {
    id: i64,
    pending: Option<PendingMap>,
}

impl PendingGuard {
    fn disarm(mut self) {
        self.pending = None;
    }
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        if let Some(ref pending) = self.pending {
            if let Ok(mut map) = pending.try_lock() {
                map.remove(&self.id);
            }
            // If lock is held, the entry leaks. This is acceptable:
            // it only happens during concurrent cancellation, and the
            // reader loop's EOF cleanup (map.clear()) will reclaim it.
        }
    }
}
