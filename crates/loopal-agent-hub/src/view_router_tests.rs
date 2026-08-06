use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, AgentEventPayload, ResolveSource};
use loopal_view_state::ViewStateReducer;
use tokio::sync::{Mutex, mpsc};

use super::handle_snapshot;
use crate::pending_relay::PendingRemoteQuestionInfo;
use crate::{Hub, HubUplink};

#[tokio::test]
async fn remote_snapshot_reads_authority_after_acquiring_reducer() {
    let interaction_id = "interaction-token";
    let request = AgentEventPayload::UserQuestionRequest {
        id: interaction_id.into(),
        logical_id: "logical-id".into(),
        questions: Vec::new(),
        classifier_running: false,
    };
    let view = Arc::new(Mutex::new(ViewStateReducer::new("origin/worker")));
    view.lock().await.apply(request.clone());
    let (client_transport, _meta_transport) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(client_transport).into_listening();
    let uplink = Arc::new(HubUplink::new(connection, "destination".into()));
    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(8);
    let mut hub = Hub::new(event_tx);
    hub.remote_views
        .insert("origin/worker".into(), view.clone());
    hub.pending_remote_questions.insert(
        ("origin/worker".into(), interaction_id.into()),
        PendingRemoteQuestionInfo {
            origin_hub: "origin".into(),
            origin_agent: "worker".into(),
            qualified_agent: "origin/worker".into(),
            interaction_id: interaction_id.into(),
            logical_id: "logical-id".into(),
            request,
            uplink,
            deadline: tokio::time::Instant::now() + Duration::from_secs(60),
            forwarding: false,
        },
    );
    let hub = Arc::new(Mutex::new(hub));

    let mut reducer = view.lock().await;
    let snapshot = tokio::spawn({
        let hub = hub.clone();
        async move { handle_snapshot(&hub, serde_json::json!({"agent": "origin/worker"})).await }
    });
    tokio::task::yield_now().await;
    hub.lock().await.pending_remote_questions.clear();
    reducer.apply(AgentEventPayload::UserQuestionResolved {
        id: interaction_id.into(),
        by: ResolveSource::Manual,
    });
    drop(reducer);

    let snapshot = snapshot.await.unwrap().unwrap();
    assert!(snapshot["state"]["agent"]["conversation"]["pending_question"].is_null());
    assert!(
        view.lock()
            .await
            .state()
            .agent
            .conversation
            .pending_question
            .is_none()
    );
}
