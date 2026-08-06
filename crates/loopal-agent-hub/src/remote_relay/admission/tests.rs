use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_protocol::{AgentEvent, AgentEventPayload, UiCapabilities};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

use super::emit_question;
use crate::{Hub, HubUplink, UiSession};

fn params(token: &str, logical_id: &str) -> serde_json::Value {
    json!({
        "origin_hub": "origin",
        "agent_name": "worker",
        "timeout_ms": 30,
        "payload": AgentEventPayload::UserQuestionRequest {
            id: token.into(),
            logical_id: logical_id.into(),
            questions: Vec::new(),
            classifier_running: false,
        },
    })
}

#[tokio::test]
async fn retry_is_idempotent_distinct_request_is_rejected_and_deadline_cleans_record() {
    let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (hub_conn, _hub_rx) = Connection::new(hub_transport).into_listening();
    let (meta_conn, mut meta_rx) = Connection::new(meta_transport).into_listening();
    let uplink = Arc::new(HubUplink::new(hub_conn, "destination".into()));
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    {
        let mut h = hub.lock().await;
        h.uplink = Some(uplink.clone());
        h.set_pending_interaction_timeout(Duration::from_millis(40));
    }
    let _ui = UiSession::connect(
        hub.clone(),
        "desktop",
        UiCapabilities {
            question: true,
            ..UiCapabilities::NONE
        },
    )
    .await;
    let responder = tokio::spawn(async move {
        let Incoming::Request { id, .. } = meta_rx.recv().await.unwrap() else {
            panic!("expected cancellation relay");
        };
        meta_conn
            .respond(id, json!({"resolved": false}))
            .await
            .unwrap();
    });

    let token = Uuid::new_v4().to_string();
    let first = emit_question(&hub, &params(&token, "logical"), uplink.clone())
        .await
        .unwrap();
    assert_eq!(first["emitted"], true);
    let retry = emit_question(&hub, &params(&token, "logical"), uplink.clone())
        .await
        .unwrap();
    assert_eq!(retry["emitted"], true);
    let distinct = emit_question(&hub, &params(&Uuid::new_v4().to_string(), "other"), uplink)
        .await
        .unwrap();
    assert_eq!(distinct["emitted"], false);
    assert_eq!(hub.lock().await.pending_remote_questions.len(), 1);

    let request = event_rx.recv().await.unwrap();
    assert!(matches!(
        request.payload,
        AgentEventPayload::UserQuestionRequest { ref id, .. } if id == &token
    ));
    tokio::time::timeout(Duration::from_secs(1), async {
        while !hub.lock().await.pending_remote_questions.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("destination deadline must remove the authoritative relay record");
    responder.await.unwrap();
    let resolved = event_rx.recv().await.unwrap();
    assert!(matches!(
        resolved.payload,
        AgentEventPayload::UserQuestionResolved { ref id, .. } if id == &token
    ));
}
