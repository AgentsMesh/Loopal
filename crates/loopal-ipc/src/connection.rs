use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::debug;

use crate::connection_reader::spawn_reader_loop;
use crate::jsonrpc;
use crate::rpc_error::RpcError;
use crate::transport::Transport;

#[path = "connection/pending_guard.rs"]
mod pending_guard;
use pending_guard::PendingGuard;
#[path = "connection/write_guard.rs"]
mod write_guard;
use write_guard::FrameWriteGuard;

#[cfg(not(test))]
const FRAME_WRITE_DEADLINE: Duration = Duration::from_secs(5);
#[cfg(test)]
const FRAME_WRITE_DEADLINE: Duration = Duration::from_millis(100);
const TRANSPORT_CLOSE_DEADLINE: Duration = Duration::from_secs(2);

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

pub(crate) type PendingMap = Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, RpcError>>>>>;

pub struct Inactive;
pub struct Listening;

pub struct Connection<S = Inactive> {
    transport: Arc<dyn Transport>,
    pending: PendingMap,
    next_id: AtomicI64,
    _state: PhantomData<S>,
}

impl Connection<Inactive> {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicI64::new(1),
            _state: PhantomData,
        }
    }

    /// Start the reader loop and transition to `Listening`. Returns the raw
    /// `Incoming` receiver so callers (`agent_io_loop`, hub_server, etc.) can
    /// drive their own routing.
    pub fn into_listening(self) -> (Arc<Connection<Listening>>, mpsc::Receiver<Incoming>) {
        let rx = spawn_reader_loop(self.transport.clone(), self.pending.clone());
        (
            Arc::new(Connection {
                transport: self.transport,
                pending: self.pending,
                next_id: self.next_id,
                _state: PhantomData,
            }),
            rx,
        )
    }
}

impl Connection<Listening> {
    pub async fn send_request(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        debug!(id, method, "IPC send_request");
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let mut guard = PendingGuard {
            id,
            method: method.to_string(),
            pending: Some(self.pending.clone()),
            transport: Some(self.transport.clone()),
            request_sent: false,
        };
        let data = jsonrpc::encode_request(id, method, params);
        if let Err(e) = self.send_frame(&data).await {
            self.pending.lock().await.remove(&id);
            guard.disarm();
            return Err(e);
        }
        guard.mark_sent();

        let outcome = rx.await.map_err(|_| RpcError::ChannelDropped);
        guard.disarm();
        outcome?
    }

    pub async fn send_notification(&self, method: &str, params: Value) -> Result<(), RpcError> {
        debug!(method, "IPC send_notification");
        let data = jsonrpc::encode_notification(method, params);
        self.send_frame(&data).await
    }

    pub async fn respond(&self, id: i64, result: Value) -> Result<(), RpcError> {
        debug!(id, "IPC respond ok");
        let data = jsonrpc::encode_response(id, result);
        self.send_frame(&data).await
    }

    pub async fn respond_error(&self, id: i64, code: i64, message: &str) -> Result<(), RpcError> {
        debug!(id, code, message, "IPC respond_error");
        let data = jsonrpc::encode_error(id, code, message);
        self.send_frame(&data).await
    }

    pub fn is_connected(&self) -> bool {
        self.transport.is_connected()
    }

    pub async fn close(&self) {
        if tokio::time::timeout(TRANSPORT_CLOSE_DEADLINE, self.transport.close())
            .await
            .is_err()
        {
            tracing::warn!("timed out closing IPC transport");
        }
    }

    async fn send_frame(&self, data: &[u8]) -> Result<(), RpcError> {
        let guard = FrameWriteGuard::new(self.transport.clone());
        let result = tokio::time::timeout(FRAME_WRITE_DEADLINE, self.transport.send(data)).await;
        match result {
            Ok(Ok(())) => {
                guard.disarm();
                Ok(())
            }
            Ok(Err(error)) => {
                if close_failed_write(&self.transport).await {
                    guard.disarm();
                }
                Err(RpcError::Transport(error.to_string()))
            }
            Err(_) => {
                if close_failed_write(&self.transport).await {
                    guard.disarm();
                }
                Err(RpcError::Transport(format!(
                    "IPC frame write timed out after {FRAME_WRITE_DEADLINE:?}"
                )))
            }
        }
    }
}

async fn close_failed_write(transport: &Arc<dyn Transport>) -> bool {
    if tokio::time::timeout(TRANSPORT_CLOSE_DEADLINE, transport.close())
        .await
        .is_err()
    {
        tracing::warn!("timed out closing IPC transport after frame write failure");
        false
    } else {
        true
    }
}

#[cfg(test)]
#[path = "connection/tests.rs"]
mod tests;
