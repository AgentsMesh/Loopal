use std::time::Duration;
use std::{error::Error, fmt};

use futures::future::join_all;
use loopal_ipc::protocol::methods;
use loopal_protocol::INTERACTION_RPC_COMPLETION_GRACE;

use crate::session_hub::SharedSession;
use crate::shared_session::ClientConnectionLease;

// Event delivery is part of the interaction completion path. Reuse its
// protocol-level grace period so transport policy cannot drift by layer.
pub(crate) const EVENT_DELIVERY_DEADLINE: Duration = INTERACTION_RPC_COMPLETION_GRACE;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DeliveryError {
    NoConnections,
    AllConnectionsFailed { attempted: usize },
    PrimaryConnectionFailed { client_id: String },
}

impl fmt::Display for DeliveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoConnections => formatter.write_str("agent event has no connected clients"),
            Self::AllConnectionsFailed { attempted } => write!(
                formatter,
                "agent event delivery failed for all {attempted} connected clients"
            ),
            Self::PrimaryConnectionFailed { client_id } => write!(
                formatter,
                "agent event delivery failed for primary client {client_id}"
            ),
        }
    }
}

impl Error for DeliveryError {}

pub(crate) async fn deliver(
    session: &SharedSession,
    params: serde_json::Value,
) -> Result<(), DeliveryError> {
    let leases = session.connection_leases().await;
    if leases.is_empty() {
        return Err(DeliveryError::NoConnections);
    }

    let attempted = leases.len();
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

    if failed.len() == attempted {
        return Err(DeliveryError::AllConnectionsFailed { attempted });
    }
    if let Some(primary) = failed.iter().find(|lease| lease.is_primary) {
        return Err(DeliveryError::PrimaryConnectionFailed {
            client_id: primary.id.clone(),
        });
    }

    Ok(())
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
