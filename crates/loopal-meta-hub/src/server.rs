//! MetaHub TCP server — accepts incoming Sub-Hub connections.

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::Mutex;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::jsonrpc::INVALID_REQUEST;
use loopal_ipc::protocol::methods;
use loopal_ipc::tcp::TcpTransport;

use crate::io_loop::meta_hub_io_loop;
use crate::meta_hub::MetaHub;

/// Start MetaHub TCP listener on the given address. Returns listener + auth token.
pub async fn start_meta_listener(addr: &str) -> anyhow::Result<(TcpListener, String)> {
    let listener = TcpListener::bind(addr).await?;
    let token = uuid::Uuid::new_v4().to_string();
    let local_addr = listener.local_addr()?;
    tracing::info!(%local_addr, "MetaHub listening");
    Ok((listener, token))
}

/// Accept loop — authenticates and registers incoming Sub-Hub connections.
pub async fn meta_accept_loop(listener: TcpListener, meta_hub: Arc<Mutex<MetaHub>>, token: String) {
    loop {
        let (stream, addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(error = %e, "accept failed");
                continue;
            }
        };

        tracing::info!(%addr, "incoming Sub-Hub connection");
        let mh = meta_hub.clone();
        let token = token.clone();

        tokio::spawn(async move {
            let transport = Arc::new(TcpTransport::new(stream));
            let conn = Arc::new(Connection::new(transport));
            let mut rx = conn.start();

            match wait_for_meta_register(&conn, &mut rx, &token).await {
                Ok((name, capabilities)) => {
                    // Register in HubRegistry
                    {
                        let mut mh = mh.lock().await;
                        if let Err(e) = mh.registry.register(&name, conn.clone(), capabilities) {
                            tracing::warn!(hub = %name, error = %e, "registration failed");
                            return;
                        }
                    }
                    tracing::info!(hub = %name, "Sub-Hub registered");

                    // Start IO loop for this Sub-Hub
                    meta_hub_io_loop(mh, conn, rx, name).await;
                }
                Err(e) => {
                    tracing::warn!(%addr, error = %e, "registration handshake failed");
                }
            }
        });
    }
}

/// Wait for `meta/register` request, validate token, extract hub name.
async fn wait_for_meta_register(
    conn: &Arc<Connection>,
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    expected_token: &str,
) -> anyhow::Result<(String, Vec<String>)> {
    while let Some(msg) = rx.recv().await {
        if let Incoming::Request { id, method, params } = msg
            && method == methods::META_REGISTER.name
        {
            let client_token = params["token"].as_str().unwrap_or("");
            if client_token != expected_token {
                let _ = conn
                    .respond_error(id, INVALID_REQUEST, "invalid token")
                    .await;
                anyhow::bail!("invalid token");
            }
            let name = params["name"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing 'name' field"))?
                .to_string();
            let capabilities: Vec<String> = params["capabilities"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let _ = conn.respond(id, serde_json::json!({"ok": true})).await;
            return Ok((name, capabilities));
        }
    }
    anyhow::bail!("connection closed before meta/register");
}
