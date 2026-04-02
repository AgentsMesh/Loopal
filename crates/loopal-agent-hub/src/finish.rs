//! Completion delivery — handles local and cross-hub parent notification.

use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_protocol::{Envelope, QualifiedAddress};

use crate::hub::Hub;

/// Emit agent finished, unregister, and deliver completion to parent.
///
/// Handles both local parents (via completion_tx) and remote parents
/// (via MetaHub uplink). Called after the agent IO loop exits.
pub(crate) async fn finish_and_deliver(hub: &Arc<Mutex<Hub>>, name: &str, output: Option<String>) {
    let output_text = output.as_deref().unwrap_or("(no output)").to_string();

    let (pending, uplink, parent_name) = {
        let mut h = hub.lock().await;
        let parent = h
            .registry
            .agent_info(name)
            .and_then(|info| info.parent.clone());
        let pending = h.registry.emit_agent_finished(name, output);
        h.registry.unregister_connection(name);
        (pending, h.uplink.clone(), parent)
    };

    if let Some((tx, envelope)) = pending {
        if tx.send(envelope).await.is_err() {
            tracing::warn!(agent = %name, "parent completion channel closed");
        }
    } else if let Some(parent) = parent_name {
        let addr = QualifiedAddress::parse(&parent);
        if addr.is_remote() {
            if let Some(ul) = uplink {
                let content =
                    format!("<agent-result name=\"{name}\">\n{output_text}\n</agent-result>");
                let envelope = Envelope::new(
                    loopal_protocol::MessageSource::System("agent-completed".into()),
                    &parent,
                    content,
                );
                if let Err(e) = ul.route(&envelope).await {
                    tracing::warn!(agent = %name, parent = %parent, error = %e,
                        "failed to deliver completion to remote parent");
                }
            }
        }
    }
}
