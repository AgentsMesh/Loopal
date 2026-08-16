use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use tokio::sync::Mutex;
use tracing::warn;

use super::cleanup::{InteractionKind, remove_if_current, schedule_timeout};
use super::completion::complete_detached;
use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::hub::Hub;

pub(super) struct PermissionDelivery {
    pub(super) event: Box<PreparedAuthoritativeEvent>,
    pub(super) agent_conn: Arc<Connection<Listening>>,
    pub(super) agent_ipc_id: i64,
    pub(super) agent_name: String,
    pub(super) tool_call_id: String,
    pub(super) interaction_id: String,
    pub(super) timeout: std::time::Duration,
}

type CoordinatorFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

pub(super) async fn coordinate(hub: &Arc<Mutex<Hub>>, delivery: PermissionDelivery) {
    coordinate_spawn(hub, delivery, tokio::spawn).await;
}

async fn coordinate_spawn(
    hub: &Arc<Mutex<Hub>>,
    delivery: PermissionDelivery,
    spawn: impl FnOnce(CoordinatorFuture) -> tokio::task::JoinHandle<()>,
) {
    let PermissionDelivery {
        mut event,
        agent_conn,
        agent_ipc_id,
        agent_name,
        tool_call_id,
        interaction_id,
        timeout,
    } = delivery;
    let delivery_hub = hub.clone();
    let delivery_conn = agent_conn.clone();
    let delivery_agent = agent_name.clone();
    let delivery_logical_id = tool_call_id.clone();
    let delivery_interaction_id = interaction_id.clone();
    let coordinator = spawn(Box::pin(async move {
        match event.deliver().await {
            Ok(()) => schedule_timeout(
                &delivery_hub,
                InteractionKind::Permission,
                delivery_agent,
                delivery_logical_id,
                delivery_interaction_id,
                timeout,
            ),
            Err(error) => {
                warn!(agent = %delivery_agent, %error, "permission event admission failed");
                if remove_if_current(
                    &delivery_hub,
                    InteractionKind::Permission,
                    &delivery_agent,
                    &delivery_logical_id,
                    &delivery_interaction_id,
                )
                .await
                {
                    deny(delivery_conn, agent_ipc_id);
                }
            }
        }
    }));
    if let Err(error) = coordinator.await {
        tracing::error!(agent = %agent_name, %error, "permission admission coordinator failed");
        hub.lock().await.shutdown_signal.notify_one();
        if remove_if_current(
            hub,
            InteractionKind::Permission,
            &agent_name,
            &tool_call_id,
            &interaction_id,
        )
        .await
        {
            deny(agent_conn, agent_ipc_id);
        }
    }
}

pub(super) fn deny(connection: Arc<Connection<Listening>>, request_id: i64) {
    complete_detached(
        connection,
        request_id,
        serde_json::json!({"allow": false}),
        None,
    );
}

#[cfg(test)]
#[path = "permission_delivery_tests.rs"]
mod tests;
