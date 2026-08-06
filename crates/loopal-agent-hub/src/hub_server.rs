//! Hub TCP server — accepts connections from external clients.
//!
//! TCP clients must provide a valid token in `hub/register` to authenticate.
//! In-process local connections (via `connect_local`) bypass authentication.
//!
//! Register payload `role` (required):
//! - `"agent"`: client is an agent worker; handled by
//!   `agent_io::start_agent_io`.
//! - `"ui_client"`: UI observer (TUI/ACP attaching to existing Hub);
//!   handled by `tcp_ui_io::start_tcp_ui_io`.

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{info, warn};

use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::transport::Transport;

use crate::hub::Hub;

#[path = "hub_server/register.rs"]
mod register;
use register::{ClientRole, wait_for_register};
#[path = "hub_server/agent_registration.rs"]
mod agent_registration;

#[cfg(not(test))]
const REGISTER_WAIT_DEADLINE: Duration = Duration::from_secs(10);
#[cfg(test)]
const REGISTER_WAIT_DEADLINE: Duration = Duration::from_millis(50);
#[cfg(not(test))]
const REGISTER_ACK_DEADLINE: Duration = Duration::from_secs(2);
#[cfg(test)]
const REGISTER_ACK_DEADLINE: Duration = Duration::from_millis(50);
const TRANSPORT_CLOSE_DEADLINE: Duration = Duration::from_secs(2);

/// Start the Hub TCP server. Returns the listener, port, and auth token.
pub async fn start_hub_listener(
    _hub: Arc<Mutex<Hub>>,
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
    hub: Arc<Mutex<Hub>>,
    name: &str,
) -> (
    Arc<Connection<Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
) {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client_conn, client_rx) = Connection::new(client_transport).into_listening();
    let (server_conn, server_rx) = Connection::new(server_transport).into_listening();
    let dispatcher = Arc::new(crate::dispatch::build_hub_dispatcher(hub.clone()));
    crate::agent_io::start_agent_io(hub, dispatcher, name, server_conn, server_rx, None);
    (client_conn, client_rx)
}

/// Accept loop — authenticates TCP clients with token.
pub async fn accept_loop(listener: TcpListener, hub: Arc<Mutex<Hub>>, token: String) {
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
            let (conn, mut rx) = Connection::new(transport).into_listening();

            match receive_register(&conn, &mut rx, &token).await {
                Ok(result) => {
                    info!(client = %result.name, role = ?result.role,
                        "Hub: TCP client authenticated and registered");
                    let (tx, owned_rx) = tokio::sync::mpsc::channel(256);
                    tokio::spawn(async move {
                        while let Some(msg) = rx.recv().await {
                            if tx.send(msg).await.is_err() {
                                break;
                            }
                        }
                    });
                    match result.role {
                        ClientRole::Agent => {
                            if let Err(error) = agent_registration::reserve_ack_and_start(
                                hub, conn, owned_rx, result,
                            )
                            .await
                            {
                                warn!(%error, "Hub: TCP agent registration failed");
                            }
                        }
                        ClientRole::UiClient => {
                            // A capability lease becomes authoritative only
                            // after its registration ACK is written successfully.
                            if acknowledge_register(&conn, &result).await.is_err() {
                                close_bounded(&conn).await;
                                return;
                            }
                            crate::tcp_ui_io::start_tcp_ui_io(
                                hub,
                                &result.name,
                                conn,
                                owned_rx,
                                result.capabilities,
                                result.lease_id,
                            )
                            .await;
                        }
                    }
                }
                Err(e) => {
                    warn!(%addr, error = %e, "Hub: TCP client rejected");
                }
            }
        });
    }
}

async fn acknowledge_register(
    conn: &Connection<Listening>,
    result: &register::RegisterResult,
) -> Result<(), String> {
    let result = match tokio::time::timeout(
        REGISTER_ACK_DEADLINE,
        conn.respond(
            result.request_id,
            serde_json::json!({
                "ok": true,
                "lease_id": result.lease_id,
                "capabilities": result.capabilities,
            }),
        ),
    )
    .await
    {
        Ok(result) => result.map_err(|error| error.to_string()),
        Err(_) => Err("hub/register acknowledgement timed out".to_string()),
    };
    if result.is_err() {
        close_bounded(conn).await;
    }
    result
}

async fn receive_register(
    conn: &Arc<Connection<Listening>>,
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    token: &str,
) -> Result<register::RegisterResult, String> {
    let result = match tokio::time::timeout(
        REGISTER_WAIT_DEADLINE,
        wait_for_register(conn, rx, token),
    )
    .await
    {
        Ok(result) => result.map_err(|error| error.to_string()),
        Err(_) => Err("hub/register timed out".to_string()),
    };
    if result.is_err() {
        close_bounded(conn).await;
    }
    result
}

async fn close_bounded(conn: &Connection<Listening>) {
    if tokio::time::timeout(TRANSPORT_CLOSE_DEADLINE, conn.close())
        .await
        .is_err()
    {
        warn!("Hub TCP transport close timed out");
    }
}

#[cfg(test)]
#[path = "hub_server/tests.rs"]
mod tests;
