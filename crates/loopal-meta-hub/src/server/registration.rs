use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::jsonrpc::INVALID_REQUEST;

use crate::MetaHub;

/// Reserve a name, write the registration ACK, then make the connection
/// routable. A failed or cancelled ACK never leaves an authoritative entry.
pub async fn register_acknowledged_connection(
    meta_hub: &Arc<Mutex<MetaHub>>,
    conn: &Arc<Connection<Listening>>,
    request_id: i64,
    name: &str,
    capabilities: Vec<String>,
    ack_deadline: Duration,
) -> Result<(), String> {
    let reservation = meta_hub
        .lock()
        .await
        .registry
        .reserve_registration(name, conn.clone());
    if let Err(error) = reservation {
        let _ = respond_error_bounded(conn, request_id, &error, ack_deadline).await;
        super::close_bounded(conn).await;
        return Err(error);
    }

    let mut guard = RegistrationGuard::new(meta_hub.clone(), name.to_string(), conn.clone());
    let ack = tokio::time::timeout(
        ack_deadline,
        conn.respond(request_id, serde_json::json!({"ok": true})),
    )
    .await;
    match ack {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            guard.cancel().await;
            super::close_bounded(conn).await;
            return Err(format!("meta/register acknowledgement failed: {error}"));
        }
        Err(_) => {
            guard.cancel().await;
            super::close_bounded(conn).await;
            return Err("meta/register acknowledgement timed out".into());
        }
    }

    let activation = meta_hub
        .lock()
        .await
        .registry
        .activate_registration(name, conn, capabilities);
    if activation.is_ok() {
        guard.disarm();
    } else {
        guard.cancel().await;
        super::close_bounded(conn).await;
    }
    activation
}

struct RegistrationGuard {
    meta_hub: Arc<Mutex<MetaHub>>,
    name: String,
    conn: Arc<Connection<Listening>>,
    armed: bool,
}

impl RegistrationGuard {
    fn new(meta_hub: Arc<Mutex<MetaHub>>, name: String, conn: Arc<Connection<Listening>>) -> Self {
        Self {
            meta_hub,
            name,
            conn,
            armed: true,
        }
    }

    async fn cancel(&mut self) {
        if self.armed {
            self.meta_hub
                .lock()
                .await
                .registry
                .cancel_registration(&self.name, &self.conn);
            self.armed = false;
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RegistrationGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let meta_hub = self.meta_hub.clone();
        let name = self.name.clone();
        let conn = self.conn.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                meta_hub
                    .lock()
                    .await
                    .registry
                    .cancel_registration(&name, &conn);
                super::close_bounded(&conn).await;
            });
        }
    }
}

async fn respond_error_bounded(
    conn: &Connection<Listening>,
    id: i64,
    error: &str,
    deadline: Duration,
) -> bool {
    matches!(
        tokio::time::timeout(deadline, conn.respond_error(id, INVALID_REQUEST, error),).await,
        Ok(Ok(()))
    )
}
