use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::Connection;
use loopal_protocol::{AgentEventPayload, PermissionIntent, PermissionIntentRequest};
use tokio::sync::{Mutex, mpsc};

use super::{cancel_pending_request, cleanup_pending_for_uplink};
use crate::pending_relay::types::InteractionAudience;
use crate::pending_relay::{PendingPermissionInfo, PendingQuestionInfo, PendingRemoteQuestionInfo};
use crate::{Hub, HubUplink};

fn permission_intent() -> PermissionIntent {
    let request = PermissionIntentRequest::create(
        "reused",
        "Bash",
        serde_json::json!({}),
        serde_json::json!({}),
        serde_json::json!({"type": "object"}),
        None,
    )
    .unwrap();
    PermissionIntent::bind(request.intent_seed, 1, 1, "new-token").unwrap()
}

#[tokio::test]
async fn stale_cancel_request_cannot_remove_reconnected_generation() {
    let (_old_peer, old_transport) = loopal_ipc::duplex_pair();
    let (old_conn, _old_rx) = Connection::new(old_transport).into_listening();
    let (_new_peer, new_transport) = loopal_ipc::duplex_pair();
    let (new_conn, _new_rx) = Connection::new(new_transport).into_listening();
    let (event_tx, _event_rx) = mpsc::channel(8);
    let mut hub = Hub::new(event_tx);
    hub.pending_permissions.insert(
        ("main".into(), "reused".into()),
        PendingPermissionInfo {
            execution: crate::types::AgentExecutionRef::local("main", 1),
            agent_conn: new_conn.clone(),
            agent_ipc_id: 1,
            agent_name: "main".into(),
            interaction_id: "new-token".into(),
            logical_id: "reused".into(),
            tool_name: "Bash".into(),
            permission_intent: permission_intent(),
        },
    );
    let hub = Arc::new(Mutex::new(hub));

    assert!(!cancel_pending_request(&hub, "main", &old_conn, 1).await);
    assert!(
        hub.lock()
            .await
            .pending_permissions
            .contains_key(&("main".into(), "reused".into()))
    );
    assert!(cancel_pending_request(&hub, "main", &new_conn, 1).await);
    assert!(hub.lock().await.pending_permissions.is_empty());
}

#[tokio::test]
async fn uplink_cleanup_is_scoped_to_exact_generation() {
    let (_old_meta, old_transport) = loopal_ipc::duplex_pair();
    let (old_conn, _old_rx) = Connection::new(old_transport).into_listening();
    let old_uplink = Arc::new(HubUplink::new(old_conn, "hub".into()));
    let (_new_meta, new_transport) = loopal_ipc::duplex_pair();
    let (new_conn, _new_rx) = Connection::new(new_transport).into_listening();
    let new_uplink = Arc::new(HubUplink::new(new_conn, "hub".into()));
    let (_agent_peer, agent_transport) = loopal_ipc::duplex_pair();
    let (agent_conn, _agent_rx) = Connection::new(agent_transport).into_listening();
    let (event_tx, _event_rx) = mpsc::channel(8);
    let mut hub = Hub::new(event_tx);
    hub.uplink = Some(new_uplink.clone());
    hub.pending_questions.insert(
        ("worker".into(), "logical".into()),
        PendingQuestionInfo {
            agent_conn,
            agent_ipc_id: 1,
            agent_name: "worker".into(),
            interaction_id: "new-token".into(),
            logical_id: "logical".into(),
            audience: InteractionAudience::RemoteUi {
                target_hub: "parent".into(),
                uplink: new_uplink.clone(),
            },
        },
    );
    hub.pending_remote_questions.insert(
        ("origin/worker".into(), "destination-token".into()),
        PendingRemoteQuestionInfo {
            origin_hub: "origin".into(),
            origin_agent: "worker".into(),
            qualified_agent: "origin/worker".into(),
            interaction_id: "destination-token".into(),
            logical_id: "destination-logical".into(),
            request: AgentEventPayload::UserQuestionRequest {
                id: "destination-token".into(),
                logical_id: "destination-logical".into(),
                questions: Vec::new(),
                classifier_running: false,
            },
            uplink: new_uplink,
            deadline: tokio::time::Instant::now() + Duration::from_secs(60),
            forwarding: false,
        },
    );
    let hub = Arc::new(Mutex::new(hub));

    cleanup_pending_for_uplink(&hub, &old_uplink).await;
    let h = hub.lock().await;
    assert_eq!(h.pending_questions.len(), 1);
    assert_eq!(h.pending_remote_questions.len(), 1);
}
