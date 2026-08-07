//! Completion delivery — handles local and cross-hub parent notification.

use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentCompletion, Envelope, MessageSource, QualifiedAddress};

use crate::hub::Hub;

/// Emit agent finished, unregister, deliver completion to parent, and close the
/// connection so the child process receives EOF on stdin and can exit.
///
/// Handles both local parents (via completion_tx) and remote parents
/// (via MetaHub uplink). Called after the agent IO loop exits.
pub async fn finish_and_deliver(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    completion: AgentCompletion,
    conn: &Arc<Connection<Listening>>,
) {
    let output_text = completion.output().to_string();

    crate::pending_relay::cleanup_pending_for_agent_connection(hub, name, conn).await;

    let finished = {
        let mut h = hub.lock().await;
        if !h.registry.is_current_connection(name, conn) {
            None
        } else {
            h.session_permission_grants
                .retain(|(agent, _)| agent != name);
            let parent = h
                .registry
                .agent_info(name)
                .and_then(|info| info.parent.clone());
            let notify_parent = h.registry.notifies_parent_on_completion(name);
            let pending = h.registry.emit_agent_completion(name, completion.clone());
            let spawn_registry = h.spawn_registry.clone();
            let was_root = spawn_registry.is_root(name);
            spawn_registry.unregister(name);
            let removed = h.registry.unregister_connection_if_current(name, conn);
            debug_assert!(
                removed,
                "connection generation changed while Hub was locked"
            );
            Some((
                pending,
                h.uplink.clone(),
                parent,
                notify_parent,
                was_root,
                h.mcp_service.clone(),
                h.shutdown_signal.clone(),
            ))
        }
    };

    let Some((mut pending, uplink, parent_addr, notify_parent, was_root, mcp_service, shutdown)) =
        finished
    else {
        tracing::debug!(agent = %name, "ignoring stale agent IO-loop teardown");
        conn.close().await;
        return;
    };

    if let Err(error) = pending.deliver_events().await {
        tracing::error!(agent = %name, %error, "authoritative completion event queue closed");
        shutdown.notify_one();
    }

    mcp_service.on_agent_detach(name, was_root).await;

    if let Some((tx, envelope)) = pending.take_parent_delivery() {
        if tx.send(envelope).await.is_err() {
            tracing::warn!(agent = %name, "parent completion channel closed");
        }
    } else if notify_parent
        && let Some(parent) = parent_addr
        && parent.is_remote()
        && let Some(ul) = uplink
    {
        let envelope = Envelope::new(
            MessageSource::AgentResult {
                child: QualifiedAddress::local(name),
            },
            parent.clone(),
            output_text,
        )
        .with_agent_completion(completion);
        if let Err(e) = ul.route(&envelope).await {
            tracing::warn!(agent = %name, parent = %parent, error = %e,
                "failed to deliver completion to remote parent");
        }
    }

    // Close the transport writer so the child process receives EOF on stdin.
    // This must happen AFTER delivery — the child's blocking stdin read will
    // return, allowing the process to exit cleanly.
    conn.close().await;
}

/// Record a completion received through the uplink before its original,
/// qualified envelope is routed to the local parent.
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
        record_cross_hub_completion_scoped(hub, child, completion, None, None, None).await,
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
    record_cross_hub_completion_scoped(hub, child, completion, Some(generation), Some(uplink), None)
        .await
}

pub(crate) async fn record_cross_hub_completion_from_uplink(
    hub: &Arc<Mutex<Hub>>,
    child: &str,
    completion: AgentCompletion,
    connection: &Arc<Connection<Listening>>,
) -> CrossHubCompletionRoute {
    record_cross_hub_completion_scoped(hub, child, completion, None, None, Some(connection)).await
}

async fn record_cross_hub_completion_scoped(
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
                // A cached completion was validated while this lease was
                // active. If the uplink reconnects before the spawn
                // coordinator drains it, the generation-bound cache remains
                // authoritative, but no old-lease quarantine is needed.
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
mod tests {
    use std::time::Duration;

    use loopal_protocol::{AgentEvent, AgentEventPayload};
    use tokio::sync::mpsc;

    use super::*;
    use crate::pending_relay::PendingPlanApprovalInfo;

    #[tokio::test]
    async fn completion_events_backpressure_in_order_without_holding_the_hub_lock() {
        let (event_tx, mut event_rx) = mpsc::channel(1);
        event_tx
            .send(AgentEvent::root(AgentEventPayload::Running))
            .await
            .unwrap();
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        let (_peer, transport) = loopal_ipc::duplex_pair();
        let (connection, _incoming) = Connection::new(transport).into_listening();
        hub.lock()
            .await
            .registry
            .register_connection("worker", connection.clone())
            .unwrap();

        let finishing = tokio::spawn({
            let hub = hub.clone();
            let connection = connection.clone();
            async move {
                finish_and_deliver(
                    &hub,
                    "worker",
                    AgentCompletion::new("error", Some("provider failed".into())),
                    &connection,
                )
                .await;
            }
        });
        tokio::task::yield_now().await;
        assert!(
            !finishing.is_finished(),
            "completion must wait for authoritative event queue capacity"
        );
        let guard = tokio::time::timeout(Duration::from_millis(100), hub.lock())
            .await
            .expect("completion backpressure must not hold the Hub lock");
        drop(guard);

        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::Running
        ));
        let error = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
            .await
            .expect("synthetic Error enqueue timed out")
            .unwrap();
        assert!(matches!(
            error.payload,
            AgentEventPayload::Error { ref message } if message == "provider failed"
        ));
        let finished = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
            .await
            .expect("Finished enqueue timed out")
            .unwrap();
        assert!(matches!(finished.payload, AgentEventPayload::Finished));
        assert_eq!(error.routing_generation, finished.routing_generation);
        assert!(error.routing_generation.is_some());
        tokio::time::timeout(Duration::from_millis(100), finishing)
            .await
            .expect("completion delivery did not finish")
            .unwrap();
    }

    #[tokio::test]
    async fn stale_finish_does_not_detach_or_clean_reconnected_agent() {
        let (_old_peer_transport, old_hub_transport) = loopal_ipc::duplex_pair();
        let (old_hub, _old_rx) = Connection::new(old_hub_transport).into_listening();
        let (_new_peer_transport, new_hub_transport) = loopal_ipc::duplex_pair();
        let (new_hub, _new_rx) = Connection::new(new_hub_transport).into_listening();
        let (event_tx, _event_rx) = mpsc::channel(8);
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        {
            let mut h = hub.lock().await;
            h.registry
                .register_connection("worker", old_hub.clone())
                .unwrap();
            h.registry.unregister_connection("worker");
            h.registry
                .register_connection("worker", new_hub.clone())
                .unwrap();
            h.pending_plan_approvals.insert(
                ("worker".into(), "reused".into()),
                PendingPlanApprovalInfo {
                    agent_conn: new_hub.clone(),
                    agent_ipc_id: 1,
                    agent_name: "worker".into(),
                    interaction_id: "new-generation-token".into(),
                    logical_id: "reused".into(),
                },
            );
            h.session_permission_grants
                .insert(("worker".into(), "Bash".into()));
        }

        finish_and_deliver(
            &hub,
            "worker",
            AgentCompletion::goal(Some("stale output".into())),
            &old_hub,
        )
        .await;

        let h = hub.lock().await;
        assert!(Arc::ptr_eq(
            &h.registry.get_agent_connection("worker").unwrap(),
            &new_hub
        ));
        assert!(
            h.pending_plan_approvals
                .contains_key(&("worker".into(), "reused".into()))
        );
        assert!(
            h.session_permission_grants
                .contains(&("worker".into(), "Bash".into()))
        );
        assert_eq!(h.registry.completion_output("worker"), None);
    }

    #[tokio::test]
    async fn parent_reconnect_between_completion_commit_and_route_cannot_receive_old_result() {
        let (event_tx, _event_rx) = mpsc::channel(8);
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        let (_old_peer_transport, old_hub_transport) = loopal_ipc::duplex_pair();
        let (old_parent, _old_parent_rx) = Connection::new(old_hub_transport).into_listening();
        {
            let mut h = hub.lock().await;
            h.registry
                .register_connection("parent", old_parent)
                .unwrap();
            h.registry
                .register_shadow(
                    "remote-child",
                    loopal_protocol::QualifiedAddress::local("parent"),
                )
                .unwrap();
        }
        let envelope = Envelope::new(
            MessageSource::AgentResult {
                child: QualifiedAddress::local("remote-child"),
            },
            QualifiedAddress::local("parent"),
            "old result",
        );

        let route = record_cross_hub_completion_scoped(
            &hub,
            "remote-child",
            AgentCompletion::goal(Some("old result".into())),
            None,
            None,
            None,
        )
        .await;
        let expected_parent_generation = route
            .local_parent_generation()
            .expect("old parent route generation");

        let (new_peer_transport, new_hub_transport) = loopal_ipc::duplex_pair();
        let (_new_peer, mut new_parent_rx) = Connection::new(new_peer_transport).into_listening();
        let (new_parent, _new_hub_rx) = Connection::new(new_hub_transport).into_listening();
        {
            let mut h = hub.lock().await;
            h.registry.unregister_connection("parent");
            h.registry
                .register_connection("parent", new_parent)
                .unwrap();
        }

        assert!(
            !crate::uplink::reverse_route::deliver_for_generation(
                &hub,
                &envelope,
                expected_parent_generation,
            )
            .await
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), new_parent_rx.recv())
                .await
                .is_err(),
            "same-name replacement parent received the old child's envelope"
        );
    }
}
