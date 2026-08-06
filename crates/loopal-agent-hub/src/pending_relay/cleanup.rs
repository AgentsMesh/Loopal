use std::sync::{Arc, Weak};
use std::time::Duration;

use loopal_protocol::UiCapability;
use tokio::sync::Mutex;

use crate::hub::Hub;
use crate::pending_relay::completion::TerminalEventSink;

mod connection;
mod interaction;
mod store;

pub(crate) use connection::{
    cancel_pending_for_agent_connection, cleanup_pending_for_agent_connection,
};
use interaction::{CleanupReason, resolve_all};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InteractionKind {
    Permission,
    Question,
    PlanApproval,
}

/// Remove stranded interactions after an agent IO loop exits.
pub(crate) async fn cleanup_pending_for_agent(hub: &Arc<Mutex<Hub>>, agent_name: &str) {
    cleanup_agent(hub, agent_name, CleanupReason::AgentDisconnected).await;
}

async fn cleanup_agent(hub: &Arc<Mutex<Hub>>, agent_name: &str, reason: CleanupReason) {
    let (pending, terminal_sink) = {
        let mut h = hub.lock().await;
        if reason == CleanupReason::AgentDisconnected {
            h.session_permission_grants
                .retain(|(agent, _)| agent != agent_name);
        }
        let pending = store::take_for_agent(&mut h, agent_name);
        (pending, TerminalEventSink::from_hub(&h))
    };
    resolve_all(pending, terminal_sink, reason, true, true);
}

/// Clean local interactions whose final capable UI has disconnected.
pub(crate) async fn cleanup_without_capable_ui(hub: &Arc<Mutex<Hub>>) {
    let (pending, remote, terminal_sink) = {
        let mut h = hub.lock().await;
        let permission = !h.ui.has_capability(UiCapability::Permission);
        let question = !h.ui.has_capability(UiCapability::Question);
        let plan = !h.ui.has_capability(UiCapability::PlanApproval);
        let pending = store::take_unavailable(&mut h, permission, question, plan);
        let remote = if question {
            h.pending_remote_questions
                .drain()
                .map(|(_, record)| record)
                .collect()
        } else {
            Vec::new()
        };
        (pending, remote, TerminalEventSink::from_hub(&h))
    };
    resolve_all(
        pending,
        terminal_sink,
        CleanupReason::NoCapableUi,
        true,
        true,
    );
    crate::remote_relay::cancel_remote_origins(hub, remote).await;
}

/// Cancel only interactions owned by the disconnected uplink generation.
/// Pointer identity prevents an old cleanup task from touching a reconnect.
pub(crate) async fn cleanup_pending_for_uplink(
    hub: &Arc<Mutex<Hub>>,
    uplink: &Arc<crate::HubUplink>,
) {
    let (origin, destination, terminal_sink) = {
        let mut h = hub.lock().await;
        let (origin, destination) = store::take_for_uplink(&mut h, uplink);
        (origin, destination, TerminalEventSink::from_hub(&h))
    };
    resolve_all(
        origin,
        terminal_sink,
        CleanupReason::AgentDisconnected,
        true,
        false,
    );
    crate::remote_relay::resolve_remote_records(hub, destination).await;
}

/// Handle a peer's `$/cancelRequest` notification.
pub(crate) async fn cancel_pending_request(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    agent_conn: &Arc<loopal_ipc::connection::Connection<loopal_ipc::connection::Listening>>,
    agent_ipc_id: i64,
) -> bool {
    let (pending, terminal_sink) = {
        let mut h = hub.lock().await;
        let pending = store::take_by_request(&mut h, agent_name, agent_conn, agent_ipc_id);
        (pending, TerminalEventSink::from_hub(&h))
    };
    let Some(pending) = pending else {
        return false;
    };
    let pending = vec![pending];
    resolve_all(
        pending,
        terminal_sink,
        CleanupReason::RequestCancelled,
        true,
        true,
    );
    true
}

pub(super) async fn remove_if_current(
    hub: &Arc<Mutex<Hub>>,
    kind: InteractionKind,
    agent_name: &str,
    logical_id: &str,
    interaction_id: &str,
) -> bool {
    let mut h = hub.lock().await;
    store::take_if_generation(&mut h, kind, agent_name, logical_id, interaction_id).is_some()
}

pub(super) fn schedule_timeout(
    hub: &Arc<Mutex<Hub>>,
    kind: InteractionKind,
    agent_name: String,
    logical_id: String,
    interaction_id: String,
    timeout: Duration,
) {
    let hub = Arc::downgrade(hub);
    tokio::spawn(async move {
        tokio::time::sleep(timeout).await;
        expire_if_current(hub, kind, agent_name, logical_id, interaction_id).await;
    });
}

async fn expire_if_current(
    hub: Weak<Mutex<Hub>>,
    kind: InteractionKind,
    agent_name: String,
    logical_id: String,
    interaction_id: String,
) {
    let Some(hub) = hub.upgrade() else {
        return;
    };
    let (pending, terminal_sink) = {
        let mut h = hub.lock().await;
        let pending =
            store::take_if_generation(&mut h, kind, &agent_name, &logical_id, &interaction_id);
        (pending, TerminalEventSink::from_hub(&h))
    };
    if let Some(pending) = pending {
        resolve_all(
            vec![pending],
            terminal_sink,
            CleanupReason::TimedOut,
            true,
            true,
        );
    }
}

#[cfg(test)]
#[path = "cleanup/tests.rs"]
mod tests;
