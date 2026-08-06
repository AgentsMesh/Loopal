use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::Envelope;
use tokio::sync::Mutex;

use crate::Hub;

#[cfg(not(test))]
const REVERSE_ROUTE_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(test)]
const REVERSE_ROUTE_TIMEOUT: Duration = Duration::from_millis(100);

/// Resolve under the Hub lock, then perform bounded transport I/O without it.
pub(super) async fn deliver(hub: &Arc<Mutex<Hub>>, envelope: &Envelope) -> bool {
    let route = {
        let locked = hub.lock().await;
        locked.registry.route_target(envelope)
    };
    let Ok((conn, event_tx)) = route else {
        return false;
    };
    match tokio::time::timeout(
        REVERSE_ROUTE_TIMEOUT,
        crate::routing::route_to_agent(&conn, envelope, &event_tx),
    )
    .await
    {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            tracing::warn!(target = %envelope.target, %error, "reverse route failed");
            false
        }
        Err(_) => {
            tracing::warn!(target = %envelope.target, "reverse route timed out");
            false
        }
    }
}

#[cfg(test)]
#[path = "reverse_route/tests.rs"]
mod tests;
