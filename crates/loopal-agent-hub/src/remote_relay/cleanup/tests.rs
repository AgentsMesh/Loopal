use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, ResolveSource};
use tokio::sync::{Mutex, mpsc};

use super::cancel_remote_origins;
use crate::pending_relay::PendingRemoteQuestionInfo;
use crate::{Hub, HubUplink};

fn resolved_event(id: &str) -> AgentEvent {
    AgentEvent::named(
        QualifiedAddress::local("origin/worker"),
        AgentEventPayload::UserQuestionResolved {
            id: id.into(),
            by: ResolveSource::Manual,
        },
    )
}

#[tokio::test]
async fn destination_terminal_event_precedes_upstream_cancellation() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    event_tx
        .send(resolved_event("queue-blocker"))
        .await
        .unwrap();
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (client_transport, peer_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (peer, mut peer_rx) = Connection::new(peer_transport).into_listening();
    let uplink = Arc::new(HubUplink::new(client, "destination".into()));
    let record = PendingRemoteQuestionInfo {
        origin_hub: "origin".into(),
        origin_agent: "worker".into(),
        qualified_agent: "origin/worker".into(),
        interaction_id: "interaction-token".into(),
        logical_id: "logical-id".into(),
        request: AgentEventPayload::UserQuestionRequest {
            id: "interaction-token".into(),
            logical_id: "logical-id".into(),
            questions: Vec::new(),
            classifier_running: false,
        },
        uplink,
        deadline: tokio::time::Instant::now() + Duration::from_secs(60),
        forwarding: false,
    };

    let cleanup = tokio::spawn({
        let hub = hub.clone();
        async move { cancel_remote_origins(&hub, vec![record]).await }
    });
    tokio::task::yield_now().await;
    assert!(
        peer_rx.try_recv().is_err(),
        "upstream cancellation raced Resolved"
    );

    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::UserQuestionResolved { ref id, .. } if id == "queue-blocker"
    ));
    let terminal = event_rx.recv().await.unwrap();
    assert!(matches!(
        terminal.payload,
        AgentEventPayload::UserQuestionResolved { ref id, .. } if id == "interaction-token"
    ));
    cleanup.await.unwrap();

    let Incoming::Request { id, method, params } = peer_rx.recv().await.unwrap() else {
        panic!("expected upstream cancellation relay");
    };
    assert_eq!(method, methods::META_REMOTE_RELAY.name);
    assert_eq!(params["operation"], "question_response");
    peer.respond(id, serde_json::json!({"resolved": true}))
        .await
        .unwrap();
}
