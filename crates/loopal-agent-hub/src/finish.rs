use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentCompletion, Envelope, MessageSource, QualifiedAddress};
use tokio::sync::Mutex;

use crate::hub::Hub;
use crate::types::AgentExecutionRef;

#[path = "finish/cross_hub.rs"]
mod cross_hub;

#[cfg(test)]
pub(crate) use cross_hub::CrossHubCompletionRoute;
pub use cross_hub::{cache_cross_hub_completion_if_spawning, record_cross_hub_completion};
pub(crate) use cross_hub::{
    record_cross_hub_completion_for_generation, record_cross_hub_completion_from_uplink,
};

pub async fn finish_and_deliver(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    completion: AgentCompletion,
    conn: &Arc<Connection<Listening>>,
) {
    let execution = hub
        .lock()
        .await
        .registry
        .execution_for_connection(name, conn);
    let Some(execution) = execution else {
        conn.close().await;
        return;
    };
    finish_and_deliver_exact(hub, name, completion, conn, &execution).await;
}

pub(crate) async fn finish_and_deliver_exact(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    completion: AgentCompletion,
    conn: &Arc<Connection<Listening>>,
    execution: &AgentExecutionRef,
) {
    let (redaction_seed, result_limit) = {
        let hub = hub.lock().await;
        (
            hub.final_sink_redaction_seed(),
            hub.registry.completion_result_limit(execution),
        )
    };
    let completion =
        crate::completion_guard::guard_with_result_limit(completion, &redaction_seed, result_limit);
    let output_text = completion.output().to_string();

    crate::pending_relay::cleanup_pending_for_agent_connection(hub, name, conn).await;

    let finished = {
        let mut h = hub.lock().await;
        if h.registry.execution_for_connection(name, conn).as_ref() != Some(execution) {
            None
        } else {
            h.clear_permission_grants(execution);
            h.permission_receipts.revoke_execution(execution);
            let parent = h
                .registry
                .agent_info(name)
                .and_then(|info| info.parent.clone());
            let notify_parent = h.registry.notifies_parent_on_completion(name);
            let pending = h.registry.emit_agent_completion(name, completion.clone());
            let spawn_registry = h.spawn_registry.clone();
            spawn_registry.unregister_exact(execution);
            let removed = h.registry.unregister_exact(execution);
            debug_assert!(
                removed,
                "connection generation changed while Hub was locked"
            );
            Some((
                pending,
                h.uplink.clone(),
                parent,
                notify_parent,
                h.mcp_service.clone(),
                h.shutdown_signal.clone(),
            ))
        }
    };

    let Some((mut pending, uplink, parent_addr, notify_parent, mcp_service, shutdown)) = finished
    else {
        tracing::debug!(agent = %name, "ignoring stale agent state teardown");
        hub.lock()
            .await
            .mcp_service
            .clone()
            .on_agent_detach(execution)
            .await;
        conn.close().await;
        return;
    };

    if let Err(error) = pending.deliver_events().await {
        tracing::error!(agent = %name, %error, "authoritative completion event queue closed");
        shutdown.notify_one();
    }

    mcp_service.on_agent_detach(execution).await;

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

    conn.close().await;
}

#[cfg(test)]
#[path = "finish/tests.rs"]
mod tests;
