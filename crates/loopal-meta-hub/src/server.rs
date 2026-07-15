//! MetaHub TCP server — accepts incoming Sub-Hub connections.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::TcpListener;
use tokio::sync::Mutex;

use loopal_ipc::connection::{Connection, Incoming, Listening};
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
    meta_accept_loop_with_timeout(listener, meta_hub, token, Duration::from_secs(5)).await;
}

#[doc(hidden)]
pub async fn meta_accept_loop_with_timeout(
    listener: TcpListener,
    meta_hub: Arc<Mutex<MetaHub>>,
    token: String,
    registration_timeout: Duration,
) {
    let mut health = tokio::time::interval(Duration::from_secs(15));
    health.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    health.tick().await;
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            _ = health.tick() => {
                sweep_unhealthy(&meta_hub).await;
                continue;
            }
        };
        let (stream, addr) = match accepted {
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
            let (conn, mut rx) = Connection::new(transport).into_listening();

            let registration = tokio::time::timeout(
                registration_timeout,
                wait_for_meta_register(&conn, &mut rx, &token),
            )
            .await;
            match registration {
                Ok(Ok((request_id, name, capabilities))) => {
                    if let Some(existing) = mh.lock().await.registry.connection(&name) {
                        let disconnected = async {
                            let mut poll =
                                tokio::time::interval(std::time::Duration::from_millis(10));
                            while existing.is_connected() {
                                poll.tick().await;
                            }
                        };
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_millis(500),
                            disconnected,
                        )
                        .await;
                    }
                    // Register in HubRegistry
                    let registration =
                        mh.lock()
                            .await
                            .registry
                            .register(&name, conn.clone(), capabilities);
                    if let Err(error) = registration {
                        let _ = conn
                            .respond_error(request_id, INVALID_REQUEST, &error)
                            .await;
                        tracing::warn!(hub = %name, %error, "registration failed");
                        return;
                    }
                    if conn
                        .respond(request_id, serde_json::json!({"ok": true}))
                        .await
                        .is_err()
                    {
                        mh.lock().await.registry.unregister_connection(&name, &conn);
                        return;
                    }
                    tracing::info!(hub = %name, "Sub-Hub registered");

                    // Start IO loop for this Sub-Hub
                    meta_hub_io_loop(mh, conn, rx, name).await;
                }
                Ok(Err(e)) => {
                    tracing::warn!(%addr, error = %e, "registration handshake failed");
                    conn.close().await;
                }
                Err(_) => {
                    tracing::warn!(%addr, "registration handshake timed out");
                    conn.close().await;
                }
            }
        });
    }
}

pub async fn sweep_unhealthy(meta_hub: &Arc<Mutex<MetaHub>>) {
    let stale = {
        let mut locked = meta_hub.lock().await;
        let names = locked.registry.check_health();
        names
            .into_iter()
            .filter_map(|name| locked.registry.unregister(&name))
            .collect::<Vec<_>>()
    };
    for hub in stale {
        hub.connection().close().await;
    }
}

#[doc(hidden)]
pub async fn sweep_unhealthy_at(
    meta_hub: &Arc<Mutex<MetaHub>>,
    now: Instant,
    degraded_after: Duration,
    disconnect_after: Duration,
) {
    let stale = {
        let mut locked = meta_hub.lock().await;
        let names = locked
            .registry
            .check_health_at(now, degraded_after, disconnect_after);
        names
            .into_iter()
            .filter_map(|name| locked.registry.unregister(&name))
            .collect::<Vec<_>>()
    };
    for hub in stale {
        hub.connection().close().await;
    }
}

/// Wait for `meta/register` request, validate token, extract hub name.
async fn wait_for_meta_register(
    conn: &Arc<Connection<Listening>>,
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    expected_token: &str,
) -> anyhow::Result<(i64, String, Vec<String>)> {
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

            return Ok((id, name, capabilities));
        }
    }
    anyhow::bail!("connection closed before meta/register");
}
