use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentCompletion, Envelope};
use tokio::sync::Mutex;

use crate::hub::Hub;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CrossHubCompletionRoute {
    Consumed,
    LocalParent { generation: u64 },
}

impl CrossHubCompletionRoute {
    pub(crate) fn local_parent_generation(self) -> Option<u64> {
        match self {
            Self::Consumed => None,
            Self::LocalParent { generation } => Some(generation),
        }
    }
}

pub async fn record_cross_hub_completion(
    hub: &Arc<Mutex<Hub>>,
    child: &str,
    completion: AgentCompletion,
) -> bool {
    crate::pending_relay::cleanup_pending_for_agent(hub, child).await;
    matches!(
        record_scoped(hub, child, completion, None, None, None).await,
        CrossHubCompletionRoute::LocalParent { .. }
    )
}

pub(crate) async fn record_cross_hub_completion_for_generation(
    hub: &Arc<Mutex<Hub>>,
    child: &str,
    generation: u64,
    uplink: &Arc<crate::uplink::HubUplink>,
    completion: AgentCompletion,
) -> CrossHubCompletionRoute {
    record_scoped(hub, child, completion, Some(generation), Some(uplink), None).await
}

pub(crate) async fn record_cross_hub_completion_from_uplink(
    hub: &Arc<Mutex<Hub>>,
    child: &str,
    completion: AgentCompletion,
    connection: &Arc<Connection<Listening>>,
) -> CrossHubCompletionRoute {
    record_scoped(hub, child, completion, None, None, Some(connection)).await
}

async fn record_scoped(
    hub: &Arc<Mutex<Hub>>,
    child: &str,
    completion: AgentCompletion,
    expected_generation: Option<u64>,
    expected_lease: Option<&Arc<crate::uplink::HubUplink>>,
    expected_connection: Option<&Arc<Connection<Listening>>>,
) -> CrossHubCompletionRoute {
    let (route, mut pending, shutdown) = {
        let mut h = hub.lock().await;
        if expected_generation
            .is_some_and(|generation| h.registry.generation(child) != Some(generation))
        {
            tracing::warn!(agent = %child, ?expected_generation, "stale cross-hub completion generation ignored");
            return CrossHubCompletionRoute::Consumed;
        }
        let completion_lease = if let Some(lease) = expected_lease {
            if h.uplink
                .as_ref()
                .is_some_and(|active| Arc::ptr_eq(active, lease))
            {
                if h.should_drop_quarantined_completion(child, lease.connection()) {
                    tracing::warn!(agent = %child, "quarantined cross-hub completion ignored");
                    return CrossHubCompletionRoute::Consumed;
                }
                Some(lease.clone())
            } else {
                None
            }
        } else if let Some(connection) = expected_connection {
            if !h.is_active_uplink_connection(connection)
                || h.should_drop_quarantined_completion(child, connection)
            {
                tracing::warn!(agent = %child, "stale or quarantined cross-hub completion ignored");
                return CrossHubCompletionRoute::Consumed;
            }
            h.uplink.clone()
        } else {
            None
        };
        if h.registry.completion(child).is_some() {
            if let Some(lease) = completion_lease {
                h.quarantine_shadow_name(child, lease);
            }
            tracing::warn!(agent = %child, "duplicate cross-hub completion ignored");
            return CrossHubCompletionRoute::Consumed;
        }
        let route = h
            .registry
            .local_parent_generation_for_completion(child)
            .map_or(CrossHubCompletionRoute::Consumed, |generation| {
                CrossHubCompletionRoute::LocalParent { generation }
            });
        let pending = h.registry.emit_agent_completion(child, completion);
        h.registry.unregister_connection(child);
        if let Some(lease) = completion_lease {
            h.quarantine_shadow_name(child, lease);
        }
        (route, pending, h.shutdown_signal.clone())
    };
    if let Err(error) = pending.deliver_events().await {
        tracing::error!(agent = %child, %error, "cross-hub completion event queue closed");
        shutdown.notify_one();
    }
    route
}

pub async fn cache_cross_hub_completion_if_spawning(
    hub: &Arc<Mutex<Hub>>,
    child: &str,
    completion: AgentCompletion,
    envelope: Envelope,
) -> bool {
    hub.lock()
        .await
        .cache_shadow_spawn_completion(child, completion, envelope)
}

#[cfg(test)]
#[path = "cross_hub_tests.rs"]
mod tests;
