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
pub(crate) async fn deliver(hub: &Arc<Mutex<Hub>>, envelope: &Envelope) -> bool {
    deliver_scoped(hub, envelope, None).await
}

/// Deliver only to the exact local parent generation captured atomically when
/// the child completion was committed. A same-name reconnect between terminal
/// event backpressure and routing must not receive the older child's result.
pub(crate) async fn deliver_for_generation(
    hub: &Arc<Mutex<Hub>>,
    envelope: &Envelope,
    expected_generation: u64,
) -> bool {
    deliver_scoped(hub, envelope, Some(expected_generation)).await
}

async fn deliver_scoped(
    hub: &Arc<Mutex<Hub>>,
    envelope: &Envelope,
    expected_generation: Option<u64>,
) -> bool {
    let route = {
        let locked = hub.lock().await;
        if expected_generation.is_some_and(|generation| {
            locked.registry.generation(&envelope.target.agent) != Some(generation)
        }) {
            return false;
        }
        locked.registry.route_target(envelope).map(|conn| {
            let observation =
                crate::routing::RouteObservation::from_hub(&locked, &envelope.target.agent);
            (conn, observation)
        })
    };
    let Ok((conn, observation)) = route else {
        return false;
    };
    match tokio::time::timeout(
        REVERSE_ROUTE_TIMEOUT,
        crate::routing::route_to_agent(&conn, envelope, &observation),
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
