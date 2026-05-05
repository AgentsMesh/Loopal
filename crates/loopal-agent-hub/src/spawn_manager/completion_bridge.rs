use std::sync::Arc;
use tokio::sync::mpsc;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;

/// Bridge: reads from Hub-internal channel, forwards to agent via IPC notification.
pub fn spawn_completion_bridge(
    name: &str,
    conn: Arc<Connection>,
    mut rx: mpsc::Receiver<Envelope>,
) {
    let n = name.to_string();
    tokio::spawn(async move {
        while let Some(envelope) = rx.recv().await {
            let params = match serde_json::to_value(&envelope) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(agent = %n, error = %e, "completion envelope serialization failed");
                    continue;
                }
            };
            if let Err(e) = conn
                .send_notification(methods::AGENT_MESSAGE.name, params)
                .await
            {
                tracing::warn!(agent = %n, error = %e, "completion notification IPC send failed");
            }
        }
    });
}
