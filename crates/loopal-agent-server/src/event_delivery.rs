use std::time::Duration;

use futures::future::join_all;
use loopal_ipc::protocol::methods;
use loopal_protocol::INTERACTION_RPC_COMPLETION_GRACE;

use crate::session_hub::SharedSession;
use crate::shared_session::ClientConnectionLease;

// Event delivery is part of the interaction completion path. Reuse its
// protocol-level grace period so transport policy cannot drift by layer.
pub(crate) const EVENT_DELIVERY_DEADLINE: Duration = INTERACTION_RPC_COMPLETION_GRACE;

pub(crate) async fn deliver(session: &SharedSession, params: serde_json::Value) {
    let leases = session.connection_leases().await;
    let failed = join_all(leases.into_iter().map(|lease| {
        let params = params.clone();
        async move {
            if send_bounded(&lease, params).await {
                None
            } else {
                close_bounded(&lease).await;
                Some(lease)
            }
        }
    }))
    .await
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if !failed.is_empty() {
        session.remove_failed_connections(&failed).await;
    }
}

async fn send_bounded(lease: &ClientConnectionLease, params: serde_json::Value) -> bool {
    match tokio::time::timeout(
        EVENT_DELIVERY_DEADLINE,
        lease
            .connection
            .send_notification(methods::AGENT_EVENT.name, params),
    )
    .await
    {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            tracing::warn!(client = %lease.id, %error, "agent event delivery failed");
            false
        }
        Err(_) => {
            tracing::warn!(client = %lease.id, "agent event delivery timed out");
            false
        }
    }
}

async fn close_bounded(lease: &ClientConnectionLease) {
    if tokio::time::timeout(EVENT_DELIVERY_DEADLINE, lease.connection.close())
        .await
        .is_err()
    {
        tracing::warn!(client = %lease.id, "event transport close timed out");
    }
}

#[cfg(test)]
#[path = "event_delivery/tests.rs"]
mod tests;
