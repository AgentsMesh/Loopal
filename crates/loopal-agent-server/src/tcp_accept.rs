//! TCP accept loop and connection handler for external IPC clients.

use std::sync::Arc;

use tracing::info;

use loopal_ipc::IpcListener;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::transport::Transport;

use crate::server_info;
use crate::session_hub::SessionHub;

/// Start a TCP listener on localhost with a system-assigned port.
pub(crate) async fn start_tcp_listener() -> Option<IpcListener> {
    let addr = "127.0.0.1:0".parse().unwrap();
    match IpcListener::bind(addr).await {
        Ok(listener) => {
            if let Err(e) = server_info::write_server_info(listener.port(), listener.token()) {
                tracing::warn!("failed to write server info: {e}");
            }
            info!(port = listener.port(), "TCP listener ready");
            Some(listener)
        }
        Err(e) => {
            tracing::warn!("TCP listener disabled: {e}");
            None
        }
    }
}

/// Accept loop: spawns a task per incoming TCP connection.
pub(crate) async fn accept_loop(listener: IpcListener, hub: Arc<SessionHub>) -> anyhow::Result<()> {
    let token = listener.token().to_string();
    loop {
        let (transport, addr) = listener.accept().await?;
        info!(%addr, "TCP client connected");
        let conn = Arc::new(Connection::new(Arc::new(transport) as Arc<dyn Transport>));
        let rx = conn.start();
        let token = token.clone();
        let hub = hub.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_tcp_connection(conn, rx, &token, &hub).await {
                tracing::warn!(%addr, error = %e, "TCP client error");
            }
            info!(%addr, "TCP client disconnected");
        });
    }
}

/// Handle a TCP client: verify token, then use shared dispatch loop.
async fn handle_tcp_connection(
    connection: Arc<Connection>,
    mut incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
    expected_token: &str,
    hub: &SessionHub,
) -> anyhow::Result<()> {
    crate::server::wait_for_initialize_with_token(
        &connection,
        &mut incoming_rx,
        Some(expected_token),
    )
    .await?;
    crate::server::dispatch_loop(connection, incoming_rx, hub, true).await
}
