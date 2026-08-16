use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, AgentEventPayload, PermissionIntent, PermissionIntentRequest};
use tokio::sync::{Mutex, mpsc};

use super::{PermissionDelivery, coordinate_spawn};
use crate::Hub;
use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::pending_relay::types::PendingPermissionInfo;
use crate::types::AgentExecutionRef;

fn connection() -> Arc<Connection<loopal_ipc::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    Connection::new(transport).into_listening().0
}

fn permission(connection: Arc<Connection<loopal_ipc::Listening>>) -> PendingPermissionInfo {
    let request = PermissionIntentRequest::create(
        "logical",
        "Bash",
        serde_json::json!({}),
        serde_json::json!({}),
        serde_json::json!({"type": "object"}),
        None,
    )
    .unwrap();
    PendingPermissionInfo {
        execution: AgentExecutionRef::local("main", 1),
        agent_conn: connection,
        agent_ipc_id: 7,
        agent_name: "main".into(),
        interaction_id: "token".into(),
        logical_id: "logical".into(),
        tool_name: "Bash".into(),
        permission_intent: PermissionIntent::bind(request.intent_seed, 1, 1, "token").unwrap(),
    }
}

#[tokio::test]
async fn join_failure_notifies_shutdown_removes_current_and_denies() {
    let (events, _rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let connection = connection();
    let pending = permission(connection.clone());
    hub.lock()
        .await
        .pending_permissions
        .insert(("main".into(), "logical".into()), pending);
    let event = PreparedAuthoritativeEvent::from_hub(
        &*hub.lock().await,
        AgentEvent::root(AgentEventPayload::Running),
    );
    let delivery = PermissionDelivery {
        event: Box::new(event),
        agent_conn: connection,
        agent_ipc_id: 7,
        agent_name: "main".into(),
        tool_call_id: "logical".into(),
        interaction_id: "token".into(),
        timeout: std::time::Duration::from_secs(1),
    };
    let shutdown = hub.lock().await.shutdown_signal.clone();
    let notified = shutdown.notified();

    coordinate_spawn(&hub, delivery, |_future| {
        tokio::spawn(async { panic!("injected coordinator failure") })
    })
    .await;

    assert!(hub.lock().await.pending_permissions.is_empty());
    tokio::time::timeout(std::time::Duration::from_secs(1), notified)
        .await
        .unwrap();
}
