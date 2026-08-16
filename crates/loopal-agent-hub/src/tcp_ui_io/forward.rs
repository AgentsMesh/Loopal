use std::sync::Arc;

use tokio::sync::{broadcast, broadcast::error::RecvError};
use tracing::{debug, warn};

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;

#[cfg(not(test))]
const UI_FORWARD_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);
#[cfg(test)]
const UI_FORWARD_DEADLINE: std::time::Duration = std::time::Duration::from_millis(200);

pub(super) async fn forward_service_events(
    client: String,
    mut events: broadcast::Receiver<loopal_workspace::ServiceNotification>,
    conn: Arc<Connection<Listening>>,
) {
    loop {
        match events.recv().await {
            Ok(event) => {
                if send_notification_bounded(&conn, event.method, event.params)
                    .await
                    .is_err()
                {
                    return;
                }
            }
            Err(RecvError::Lagged(dropped)) => {
                warn!(client, dropped, "workspace event forward lagged");
                if send_service_lag(&conn, dropped).await.is_err() {
                    return;
                }
            }
            Err(RecvError::Closed) => std::future::pending().await,
        }
    }
}

pub(super) async fn forward_events(
    client: String,
    mut events: broadcast::Receiver<AgentEvent>,
    mut resync: broadcast::Receiver<()>,
    conn: Arc<Connection<Listening>>,
) {
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(event) => {
                    let Ok(payload) = serde_json::to_value(event) else { continue };
                    if send_notification_bounded(&conn, methods::AGENT_EVENT.name, payload).await.is_err() {
                        debug!(client, "TCP UI client connection closed; stop forwarding");
                        return;
                    }
                }
                Err(RecvError::Lagged(dropped)) => {
                    warn!(client, dropped, "TCP UI forward lagged; signaling resync");
                    if send_view_resync(&conn).await.is_err() { return; }
                }
                Err(RecvError::Closed) => return,
            },
            signal = resync.recv() => match signal {
                Ok(()) | Err(RecvError::Lagged(_)) => {
                    if send_view_resync(&conn).await.is_err() { return; }
                }
                Err(RecvError::Closed) => return,
            },
        }
    }
}

async fn send_service_lag(conn: &Connection<Listening>, dropped: u64) -> Result<(), String> {
    send_notification_bounded(
        conn,
        methods::WORKSPACE_RESYNC_REQUIRED.name,
        serde_json::json!({
            "workspaceId": loopal_workspace::LOCAL_WORKSPACE_ID,
            "reason": "event_lag",
            "droppedEvents": dropped,
        }),
    )
    .await
}

async fn send_view_resync(conn: &Connection<Listening>) -> Result<(), String> {
    send_notification_bounded(
        conn,
        methods::VIEW_RESYNC_REQUIRED.name,
        serde_json::json!({}),
    )
    .await
}

async fn send_notification_bounded(
    conn: &Connection<Listening>,
    method: &str,
    params: serde_json::Value,
) -> Result<(), String> {
    match tokio::time::timeout(UI_FORWARD_DEADLINE, conn.send_notification(method, params)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => {
            warn!(
                method,
                "TCP UI notification timed out; closing lease transport"
            );
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), conn.close()).await;
            Err(format!("{method} timed out"))
        }
    }
}
