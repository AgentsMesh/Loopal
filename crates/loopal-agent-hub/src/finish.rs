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

    crate::pending_relay::cleanup_pending_for_agent(hub, name).await;

    let (pending, uplink, parent_addr, notify_parent, spawn_registry, mcp_service) = {
        let mut h = hub.lock().await;
        let parent = h
            .registry
            .agent_info(name)
            .and_then(|info| info.parent.clone());
        let notify_parent = h.registry.notifies_parent_on_completion(name);
        let pending = h.registry.emit_agent_finished(name, output);
        h.registry.unregister_connection(name);
        (
            pending,
            h.uplink.clone(),
            parent,
            notify_parent,
            h.spawn_registry.clone(),
            h.mcp_service.clone(),
        )
    };

    let was_root = spawn_registry.is_root(name);
    mcp_service.on_agent_detach(name, was_root).await;
    spawn_registry.unregister(name);

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
