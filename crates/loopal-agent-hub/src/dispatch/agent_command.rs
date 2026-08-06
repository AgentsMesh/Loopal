use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::{Connection, Listening};

#[cfg(not(test))]
pub(super) const CONTROL_DEADLINE: Duration = Duration::from_secs(5);
#[cfg(test)]
pub(super) const CONTROL_DEADLINE: Duration = Duration::from_millis(50);

#[cfg(not(test))]
pub(super) const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(2);
#[cfg(test)]
pub(super) const SHUTDOWN_DEADLINE: Duration = Duration::from_millis(50);

pub(super) async fn request(
    conn: Arc<Connection<Listening>>,
    method: &str,
    params: serde_json::Value,
    target: &str,
    action: &str,
    deadline: Duration,
) -> Result<serde_json::Value, String> {
    match tokio::time::timeout(deadline, conn.send_request(method, params)).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(error)) => Err(format!("{action} to '{target}' failed: {error}")),
        Err(_) => {
            let _ = tokio::time::timeout(Duration::from_secs(2), conn.close()).await;
            Err(format!("{action} to '{target}' timed out"))
        }
    }
}
