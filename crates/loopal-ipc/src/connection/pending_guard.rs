use std::sync::Arc;
use std::time::Duration;

use tracing::warn;

use super::PendingMap;
use crate::jsonrpc;
use crate::transport::Transport;

pub(super) struct PendingGuard {
    pub(super) id: i64,
    pub(super) method: String,
    pub(super) pending: Option<PendingMap>,
    pub(super) transport: Option<Arc<dyn Transport>>,
    pub(super) request_sent: bool,
}

impl PendingGuard {
    pub(super) fn mark_sent(&mut self) {
        self.request_sent = true;
    }

    pub(super) fn disarm(mut self) {
        self.pending = None;
        self.transport = None;
    }
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        let Some(transport) = self.transport.take() else {
            return;
        };
        let id = self.id;
        let method = std::mem::take(&mut self.method);
        let request_sent = self.request_sent;

        // A dropped request future means its caller no longer accepts a
        // response. Remove the local waiter and tell the peer so it can tear
        // down any server-side pending interaction keyed by this request ID.
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            if let Ok(mut map) = pending.try_lock() {
                map.remove(&id);
            }
            return;
        };
        runtime.spawn(async move {
            if pending.lock().await.remove(&id).is_none() {
                return;
            }
            if !request_sent {
                // AsyncWrite::write_all is not cancellation-safe. The request
                // frame may be partial, so this stream cannot safely carry a
                // cancellation frame or any subsequent JSON-RPC traffic.
                close_transport(&transport, id, "incomplete request send").await;
                return;
            }
            let data = jsonrpc::encode_notification(
                crate::protocol::methods::REQUEST_CANCEL.name,
                serde_json::json!({"id": id, "method": method}),
            );
            let result = tokio::time::timeout(Duration::from_secs(2), transport.send(&data)).await;
            match result {
                Ok(Ok(())) => return,
                Ok(Err(error)) => {
                    warn!(id, error = %error, "IPC request cancellation notification failed");
                }
                Err(_) => warn!(id, "IPC request cancellation notification timed out"),
            }
            close_transport(&transport, id, "cancellation failure").await;
        });
    }
}

async fn close_transport(transport: &Arc<dyn Transport>, id: i64, reason: &str) {
    if tokio::time::timeout(Duration::from_secs(2), transport.close())
        .await
        .is_err()
    {
        warn!(id, reason, "timed out closing IPC transport");
    }
}
