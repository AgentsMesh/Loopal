use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::{AgentEvent, UserQuestionResponse};
use tokio::sync::{Mutex, mpsc};

use super::{InteractionAudience, PendingQuestionInfo, resolve_remote_question};
use crate::{Hub, HubUplink};

#[tokio::test]
async fn remote_question_requires_exact_uplink_and_rewrites_logical_id() {
    let (active_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (active, _active_rx) = Connection::new(active_transport).into_listening();
    let (_meta, _meta_rx) = Connection::new(meta_transport).into_listening();
    let (stale_transport, _stale_peer) = loopal_ipc::duplex_pair();
    let (stale, _stale_rx) = Connection::new(stale_transport).into_listening();
    let active_uplink = Arc::new(HubUplink::new(active, "origin".into()));
    let stale_uplink = Arc::new(HubUplink::new(stale, "origin".into()));

    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_rx) = Connection::new(agent_transport).into_listening();
    let (hub_agent, mut hub_agent_rx) = Connection::new(hub_transport).into_listening();
    let pending_response = tokio::spawn(async move {
        agent
            .send_request("test/question", serde_json::json!({}))
            .await
    });
    let Incoming::Request { id, .. } = hub_agent_rx.recv().await.unwrap() else {
        panic!("expected agent request")
    };
    let (events, mut event_rx) = mpsc::channel::<AgentEvent>(8);
    let mut hub = Hub::new(events);
    hub.uplink = Some(active_uplink.clone());
    hub.pending_questions.insert(
        ("worker".into(), "logical".into()),
        PendingQuestionInfo {
            agent_conn: hub_agent,
            agent_ipc_id: id,
            agent_name: "worker".into(),
            interaction_id: "token".into(),
            logical_id: "logical".into(),
            audience: InteractionAudience::RemoteUi {
                target_hub: "destination".into(),
                uplink: active_uplink.clone(),
            },
        },
    );
    let hub = Arc::new(Mutex::new(hub));

    let stale = resolve_remote_question(
        &hub,
        "worker",
        "token",
        UserQuestionResponse::answered("token", vec!["stale".into()]),
        &stale_uplink,
    )
    .await;
    assert!(stale.unwrap_err().contains("stale uplink generation"));
    assert_eq!(hub.lock().await.pending_questions.len(), 1);

    assert!(
        resolve_remote_question(
            &hub,
            "worker",
            "token",
            UserQuestionResponse::answered("token", vec!["answer".into()]),
            &active_uplink,
        )
        .await
        .unwrap()
    );
    let response = pending_response.await.unwrap().unwrap();
    assert_eq!(response["question_id"], "logical");
    assert_eq!(response["answers"][0], "answer");
    assert!(event_rx.recv().await.is_some());
}
