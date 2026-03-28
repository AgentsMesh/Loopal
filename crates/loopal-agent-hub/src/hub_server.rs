//! Hub TCP server — accepts connections from external clients.
//!
//! TCP clients must provide a valid token in `hub/register` to authenticate.
//! In-process local connections (via `connect_local`) bypass authentication.

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{info, warn};

use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;

use crate::hub::AgentHub;

/// Start the Hub TCP server. Returns the listener, port, and auth token.
pub async fn start_hub_listener(
    _hub: Arc<Mutex<AgentHub>>,
) -> anyhow::Result<(TcpListener, u16, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let token = uuid::Uuid::new_v4().to_string();
    info!(port, "Hub TCP listener ready");
    Ok((listener, port, token))
}

/// Create an in-process "local" connection to the Hub (no TCP, no auth).
/// Returns (client_conn, incoming_rx) — caller can receive requests from Hub.
pub fn connect_local(
    hub: Arc<Mutex<AgentHub>>,
    name: &str,
) -> (Arc<Connection>, tokio::sync::mpsc::Receiver<Incoming>) {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let client_conn = Arc::new(Connection::new(client_transport));
    let server_conn = Arc::new(Connection::new(server_transport));
    let client_rx = client_conn.start();
    let server_rx = server_conn.start();
    crate::agent_io::start_agent_io(hub, name, server_conn, server_rx, false);
    (client_conn, client_rx)
}

/// Accept loop — authenticates TCP clients with token.
pub async fn accept_loop(listener: TcpListener, hub: Arc<Mutex<AgentHub>>, token: String) {
    loop {
        let (stream, addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                warn!("Hub accept error: {e}");
                continue;
            }
        };
        info!(%addr, "Hub: new TCP connection");
        let hub = hub.clone();
        let token = token.clone();
        tokio::spawn(async move {
            let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
            let conn = Arc::new(Connection::new(transport));
            let mut rx = conn.start();
            match wait_for_register(&conn, &mut rx, &token).await {
                Ok(name) => {
                    info!(agent = %name, "Hub: TCP client authenticated and registered");
                    let (tx, owned_rx) = tokio::sync::mpsc::channel(256);
                    tokio::spawn(async move {
                        while let Some(msg) = rx.recv().await {
                            if tx.send(msg).await.is_err() { break; }
                        }
                    });
                    crate::agent_io::start_agent_io(hub, &name, conn, owned_rx, false);
                }
                Err(e) => {
                    warn!(%addr, error = %e, "Hub: TCP client rejected");
                }
            }
        });
    }
}

/// Wait for `hub/register` with valid token. Returns agent name.
async fn wait_for_register(
    conn: &Arc<Connection>,
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    expected_token: &str,
) -> anyhow::Result<String> {
    loop {
        let Some(msg) = rx.recv().await else {
            anyhow::bail!("connection closed before hub/register");
        };
        if let Incoming::Request { id, method, params } = msg {
            if method == methods::HUB_REGISTER.name {
                // Validate token
                let client_token = params["token"].as_str().unwrap_or("");
                if client_token != expected_token {
                    let _ = conn
                        .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, "invalid token")
                        .await;
                    anyhow::bail!("invalid token");
                }
                let name = params["name"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("hub/register: missing 'name'"))?
                    .to_string();
                let _ = conn
                    .respond(id, serde_json::json!({"ok": true}))
                    .await;
                return Ok(name);
            }
            let _ = conn
                .respond_error(
                    id,
                    loopal_ipc::jsonrpc::INVALID_REQUEST,
                    "expected hub/register first",
                )
                .await;
        }
    }
}
