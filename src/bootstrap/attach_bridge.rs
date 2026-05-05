use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::info;

use loopal_ipc::Connection;
use loopal_ipc::TcpTransport;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload};

pub async fn connect_and_register(
    addr: &str,
    token: &str,
) -> anyhow::Result<(Arc<Connection>, mpsc::Receiver<Incoming>)> {
    let stream = TcpStream::connect(addr).await?;
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let conn = Arc::new(Connection::new(transport));
    let incoming_rx = conn.start();
    let client_name = format!("tui-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let response = conn
        .send_request(
            methods::HUB_REGISTER.name,
            serde_json::json!({
                "name": client_name,
                "token": token,
                "role": "ui_client",
            }),
        )
        .await
        .map_err(|e| anyhow::anyhow!("hub/register transport error: {e}"))?;
    if response.get("message").is_some() {
        anyhow::bail!("hub/register failed: {response}");
    }
    info!(client = %client_name, "TUI attach: registered as UI client");
    Ok((conn, incoming_rx))
}

pub fn bridge_events(
    mut incoming_rx: mpsc::Receiver<Incoming>,
    connection_lost: Arc<AtomicBool>,
) -> (mpsc::Receiver<AgentEvent>, mpsc::Receiver<()>) {
    let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(256);
    let (resync_tx, resync_rx) = mpsc::channel::<()>(8);
    tokio::spawn(async move {
        while let Some(msg) = incoming_rx.recv().await {
            match msg {
                Incoming::Notification { method, .. }
                    if method == methods::VIEW_RESYNC_REQUIRED.name =>
                {
                    let _ = resync_tx.try_send(());
                }
                Incoming::Notification { method, params }
                    if method == methods::AGENT_EVENT.name =>
                {
                    match serde_json::from_value::<AgentEvent>(params) {
                        Ok(event) => {
                            if event_tx.send(event).await.is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "TUI bridge: failed to deserialize agent/event payload, dropping"
                            );
                        }
                    }
                }
                Incoming::Notification { method, .. } => {
                    tracing::debug!(method = %method, "TUI bridge: ignoring notification");
                }
                Incoming::Request { id, method, .. } => {
                    tracing::warn!(
                        method = %method, id = ?id,
                        "TUI bridge received unexpected hub-initiated request"
                    );
                }
            }
        }
        connection_lost.store(true, Ordering::Relaxed);
        let synthetic = AgentEvent::root(AgentEventPayload::Error {
            message: "Hub connection lost. The Hub process exited or the network closed. \
                      Use /exit to leave."
                .to_string(),
        });
        let _ = event_tx.send(synthetic).await;
    });
    (event_rx, resync_rx)
}
