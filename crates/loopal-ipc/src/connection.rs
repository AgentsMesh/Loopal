use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use serde_json::Value;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::debug;

use crate::connection_reader::spawn_reader_loop;
use crate::jsonrpc;
use crate::transport::Transport;

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

pub(crate) type PendingMap = Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>;

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

    pub fn start(&self) -> mpsc::Receiver<Incoming> {
        spawn_reader_loop(self.transport.clone(), self.pending.clone())
    }

    /// Send a JSON-RPC request and wait for the response.
    /// Cancellation-safe: dropped futures clean up via `PendingGuard`.
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

        let pending = self.pending.clone();
        let guard = PendingGuard {
            id,
            pending: Some(pending),
        };
        let result = rx.await.map_err(|_| "response channel dropped".to_string());
        guard.disarm();
        result
    }

    pub async fn send_notification(&self, method: &str, params: Value) -> Result<(), String> {
        debug!(method, "IPC send_notification");
        let data = jsonrpc::encode_notification(method, params);
        self.transport
            .send(&data)
            .await
            .map_err(|e| format!("transport send failed: {e}"))
    }

    pub async fn respond(&self, id: i64, result: Value) -> Result<(), String> {
        debug!(id, "IPC respond ok");
        let data = jsonrpc::encode_response(id, result);
        self.transport
            .send(&data)
            .await
            .map_err(|e| format!("transport send failed: {e}"))
    }

    pub async fn respond_error(&self, id: i64, code: i64, message: &str) -> Result<(), String> {
        debug!(id, code, message, "IPC respond_error");
        let data = jsonrpc::encode_error(id, code, message);
        self.transport
            .send(&data)
            .await
            .map_err(|e| format!("transport send failed: {e}"))
    }

    pub fn is_connected(&self) -> bool {
        self.transport.is_connected()
    }

    /// Close the write side of the underlying transport.
    /// The remote end will see EOF; pending responses are cleaned up by the reader loop.
    pub async fn close(&self) {
        self.transport.close().await;
    }
}

/// Removes a pending request entry on drop (cancellation safety).
/// Call `disarm()` on success to skip cleanup. Uses `try_lock` in Drop
/// to avoid spawning async tasks (unsafe during runtime shutdown).
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
        if let Some(ref pending) = self.pending
            && let Ok(mut map) = pending.try_lock()
        {
            map.remove(&self.id);
        }
        // reason: if lock is held, entry leaks — reader loop cleanup reclaims it.
    }
}
