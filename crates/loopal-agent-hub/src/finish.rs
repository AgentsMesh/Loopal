//! Completion delivery — handles local and cross-hub parent notification.

use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{Envelope, MessageSource, QualifiedAddress};

use crate::hub::Hub;

/// Emit agent finished, unregister, deliver completion to parent, and close the
/// connection so the child process receives EOF on stdin and can exit.
///
/// Handles both local parents (via completion_tx) and remote parents
/// (via MetaHub uplink). Called after the agent IO loop exits.
pub async fn finish_and_deliver(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    output: Option<String>,
    conn: &Arc<Connection<Listening>>,
) {
    let output_text = output.as_deref().unwrap_or("(no output)").to_string();

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
            let pending = h.registry.emit_agent_finished(name, output);
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
            ))
        }
    };

    let Some((pending, uplink, parent_addr, notify_parent, was_root, mcp_service)) = finished
    else {
        tracing::debug!(agent = %name, "ignoring stale agent IO-loop teardown");
        conn.close().await;
        return;
    };

    mcp_service.on_agent_detach(name, was_root).await;

    if let Some((tx, envelope)) = pending {
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
        );
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
pub async fn record_cross_hub_completion(hub: &Arc<Mutex<Hub>>, child: &str, output: String) {
    crate::pending_relay::cleanup_pending_for_agent(hub, child).await;
    {
        let mut h = hub.lock().await;
        h.registry.emit_agent_finished(child, Some(output));
        h.registry.unregister_connection(child);
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::pending_relay::PendingPlanApprovalInfo;

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

        finish_and_deliver(&hub, "worker", Some("stale output".into()), &old_hub).await;

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
}
