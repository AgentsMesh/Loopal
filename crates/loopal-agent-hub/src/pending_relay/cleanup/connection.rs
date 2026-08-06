use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use tokio::sync::Mutex;

use super::interaction::{CleanupReason, resolve_all};
use super::store;
use crate::hub::Hub;
use crate::pending_relay::completion::TerminalEventSink;

/// Remove only interactions admitted by one agent connection generation.
pub(crate) async fn cleanup_pending_for_agent_connection(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    agent_conn: &Arc<Connection<Listening>>,
) {
    cleanup_agent_connection(
        hub,
        agent_name,
        agent_conn,
        CleanupReason::AgentDisconnected,
    )
    .await;
}

pub(crate) async fn cancel_pending_for_agent_connection(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    agent_conn: &Arc<Connection<Listening>>,
) {
    cleanup_agent_connection(hub, agent_name, agent_conn, CleanupReason::AgentInterrupted).await;
}

async fn cleanup_agent_connection(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    agent_conn: &Arc<Connection<Listening>>,
    reason: CleanupReason,
) {
    let (pending, terminal_sink) = {
        let mut h = hub.lock().await;
        let pending = store::take_for_agent_connection(&mut h, agent_name, agent_conn);
        (pending, TerminalEventSink::from_hub(&h))
    };
    resolve_all(pending, terminal_sink, reason, true, true);
}
