use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::{Connection, Listening};
use loopal_protocol::ControlDisposition;

#[cfg(not(test))]
pub(super) const CONTROL_DEADLINE: Duration = loopal_protocol::DEFAULT_CONTROL_RPC_TIMEOUT;
#[cfg(test)]
pub(super) const CONTROL_DEADLINE: Duration = Duration::from_millis(50);

#[cfg(not(test))]
pub(super) const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(2);
#[cfg(test)]
pub(super) const SHUTDOWN_DEADLINE: Duration = Duration::from_millis(50);

#[derive(Clone, Copy)]
pub(super) enum TimeoutDisposition {
    /// The request frame may have reached the agent. Preserve the connection
    /// and the in-flight response waiter because application state is unknown.
    PreserveAsUnknown,
    /// Shutdown owns connection teardown, so a missing acknowledgement is
    /// resolved by closing the transport.
    CloseConnection,
}

pub(super) async fn request(
    conn: Arc<Connection<Listening>>,
    method: &str,
    params: serde_json::Value,
    target: &str,
    action: &str,
    deadline: Duration,
    timeout_disposition: TimeoutDisposition,
) -> Result<serde_json::Value, String> {
    let mut pending = Box::pin({
        let conn = Arc::clone(&conn);
        let method = method.to_string();
        async move { conn.send_request(&method, params).await }
    });
    match tokio::time::timeout(deadline, pending.as_mut()).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(error)) => Err(format!("{action} to '{target}' failed: {error}")),
        Err(_) if matches!(timeout_disposition, TimeoutDisposition::CloseConnection) => {
            drop(pending);
            let _ = tokio::time::timeout(Duration::from_secs(2), conn.close()).await;
            Err(format!("{action} to '{target}' timed out"))
        }
        Err(_) => {
            tracing::warn!(
                target,
                action,
                "agent command acknowledgement timed out; application state is unknown"
            );
            let target = target.to_string();
            let action = action.to_string();
            tokio::spawn(async move {
                match pending.await {
                    Ok(_) => tracing::info!(
                        target,
                        action,
                        "agent command acknowledged after Hub deadline"
                    ),
                    Err(error) => tracing::warn!(
                        target,
                        action,
                        %error,
                        "agent command failed after Hub deadline"
                    ),
                }
            });
            serde_json::to_value(ControlDisposition::Unknown)
                .map_err(|error| format!("failed to serialize control disposition: {error}"))
        }
    }
}

pub(super) fn normalize_control_response(
    response: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let disposition = ControlDisposition::from_wire_value(response)?;
    serde_json::to_value(disposition)
        .map_err(|error| format!("failed to serialize control disposition: {error}"))
}
