use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::Connection;
use loopal_ipc::connection::{Incoming, Listening};
use loopal_protocol::{AgentEventPayload, UserQuestionResponse};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

use super::forward_question_response;
use crate::pending_relay::PendingRemoteQuestionInfo;
use crate::{Hub, HubUplink};

type Fixture = (
    Arc<Mutex<Hub>>,
    Arc<Connection<Listening>>,
    mpsc::Receiver<Incoming>,
);

async fn fixture() -> Fixture {
    let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (hub_conn, _hub_rx) = Connection::new(hub_transport).into_listening();
    let (meta_conn, meta_rx) = Connection::new(meta_transport).into_listening();
    let uplink = Arc::new(HubUplink::new(hub_conn, "destination".into()));
    let (event_tx, _event_rx) = mpsc::channel(8);
    let mut hub = Hub::new(event_tx);
    hub.uplink = Some(uplink.clone());
    hub.pending_remote_questions.insert(
        ("origin/worker".into(), "token".into()),
        PendingRemoteQuestionInfo {
            origin_hub: "origin".into(),
            origin_agent: "worker".into(),
            qualified_agent: "origin/worker".into(),
            interaction_id: "token".into(),
            logical_id: "logical".into(),
            request: AgentEventPayload::UserQuestionRequest {
                id: "token".into(),
                logical_id: "logical".into(),
                questions: Vec::new(),
                classifier_running: false,
            },
            uplink,
            deadline: tokio::time::Instant::now() + Duration::from_secs(60),
            forwarding: false,
        },
    );
    (Arc::new(Mutex::new(hub)), meta_conn, meta_rx)
}

fn response_payload(outer: &str, body: &str) -> serde_json::Value {
    json!({
        "question_id": outer,
        "response": UserQuestionResponse::answered(body, vec!["answer".into()]),
    })
}

#[tokio::test]
async fn body_id_mismatch_does_not_claim_or_forward_record() {
    let (hub, _meta, mut meta_rx) = fixture().await;
    let error = forward_question_response(
        &hub,
        "origin/worker",
        response_payload("token", "other-token"),
    )
    .await
    .unwrap_err();
    assert!(error.contains("id mismatch"));
    let h = hub.lock().await;
    assert!(!h.pending_remote_questions[&("origin/worker".into(), "token".into())].forwarding);
    drop(h);
    assert!(meta_rx.try_recv().is_err());
}

#[tokio::test]
async fn unresolved_origin_ack_keeps_destination_record_retryable() {
    let (hub, meta, mut meta_rx) = fixture().await;
    let responder = tokio::spawn(async move {
        let Incoming::Request { id, .. } = meta_rx.recv().await.unwrap() else {
            panic!("expected relay request");
        };
        meta.respond(id, json!({"resolved": false})).await.unwrap();
    });
    let resolved = super::super::forward_question_response(
        &hub,
        "origin/worker",
        response_payload("token", "token"),
    )
    .await
    .unwrap();
    responder.await.unwrap();
    assert!(!resolved);
    let h = hub.lock().await;
    let record = &h.pending_remote_questions[&("origin/worker".into(), "token".into())];
    assert!(!record.forwarding);
}

#[tokio::test]
async fn cancelled_forward_releases_generation_scoped_claim() {
    let (hub, _meta, mut meta_rx) = fixture().await;
    let forward = tokio::spawn({
        let hub = hub.clone();
        async move {
            forward_question_response(&hub, "origin/worker", response_payload("token", "token"))
                .await
        }
    });
    let request = meta_rx.recv().await.unwrap();
    assert!(matches!(request, Incoming::Request { .. }));
    assert!(
        hub.lock().await.pending_remote_questions[&("origin/worker".into(), "token".into())]
            .forwarding
    );

    forward.abort();
    let _ = forward.await;
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if !hub.lock().await.pending_remote_questions[&("origin/worker".into(), "token".into())]
                .forwarding
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("dropped relay future must release its forwarding claim");
}
