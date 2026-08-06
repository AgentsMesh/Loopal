use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{HubUplink, UiSession, start_event_loop};
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEventPayload;
use serde_json::json;

use crate::test_helpers::*;

#[tokio::test]
async fn remote_question_round_trips_through_parent_hub_ui() {
    let cluster = cluster().await;
    let ui = UiSession::connect(
        cluster.hub_a.clone(),
        "desktop-ui",
        loopal_protocol::UiCapabilities {
            permission: false,
            question: true,
            plan_approval: false,
        },
    )
    .await;
    let (remote_agent, _) =
        register_mock_agent(&cluster.hub_b, "remote-worker", Some("hub-a/main")).await;
    let request = tokio::spawn(async move {
        remote_agent
            .send_request(
                methods::AGENT_QUESTION.name,
                json!({
                    "question_id": "remote-question",
                    "questions": [{
                        "header": "Remote", "question": "Choose remote verification",
                        "options": [{"label": "Fast", "description": "Use the fast path."}],
                        "allow_multiple": false
                    }]
                }),
            )
            .await
    });
    let mut events = ui.event_rx;
    let event = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let event = events.recv().await.unwrap();
            if matches!(event.payload, AgentEventPayload::UserQuestionRequest { .. }) {
                return event;
            }
        }
    })
    .await
    .expect("remote question did not reach parent UI");
    assert_eq!(
        event.agent_name.as_ref().unwrap().agent,
        "hub-b/remote-worker",
        "Desktop must receive a qualified remote agent id"
    );
    let interaction_id = match event.payload {
        AgentEventPayload::UserQuestionRequest { id, .. } => id,
        _ => unreachable!(),
    };
    ui.client
        .connection()
        .send_request(
            methods::HUB_QUESTION_RESPONSE.name,
            json!({
                "agent_name": "hub-b/remote-worker",
                "question_id": interaction_id.clone(),
                "response": {
                    "kind": "answered", "question_id": interaction_id,
                    "answers": ["Fast"]
                }
            }),
        )
        .await
        .unwrap();
    let response = request.await.unwrap().unwrap();
    assert_eq!(response["kind"], "answered");
    assert_eq!(response["answers"][0], "Fast");
    let resolved = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let event = events.recv().await.unwrap();
            if matches!(
                event.payload,
                AgentEventPayload::UserQuestionResolved { .. }
            ) {
                return event;
            }
        }
    })
    .await
    .expect("remote question resolution did not reach parent UI");
    assert_eq!(resolved.agent_name.unwrap().agent, "hub-b/remote-worker");
}

#[tokio::test]
async fn last_question_ui_disconnect_cancels_origin_and_snapshot_reconciles() {
    let cluster = cluster().await;
    let ui = UiSession::connect(
        cluster.hub_a.clone(),
        "desktop-ui",
        loopal_protocol::UiCapabilities {
            permission: false,
            question: true,
            plan_approval: false,
        },
    )
    .await;
    let mut events = ui.event_rx.resubscribe();
    let (remote_agent, _) =
        register_mock_agent(&cluster.hub_b, "remote-worker", Some("hub-a/main")).await;
    let request = tokio::spawn(async move {
        remote_agent
            .send_request(
                methods::AGENT_QUESTION.name,
                json!({"question_id": "disconnect", "questions": []}),
            )
            .await
    });
    let interaction_id = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let event = events.recv().await.unwrap();
            if let AgentEventPayload::UserQuestionRequest { id, .. } = event.payload {
                break id;
            }
        }
    })
    .await
    .expect("remote question did not reach destination UI");

    let snapshot = loopal_agent_hub::view_router::handle_snapshot(
        &cluster.hub_a,
        json!({"agent": "hub-b/remote-worker"}),
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot["state"]["agent"]["conversation"]["pending_question"]["id"],
        interaction_id
    );
    assert_eq!(cluster.hub_a.lock().await.pending_remote_questions.len(), 1);

    drop(ui);
    let response = tokio::time::timeout(Duration::from_secs(2), request)
        .await
        .expect("origin agent must be cancelled immediately when the final UI disconnects")
        .unwrap()
        .unwrap();
    assert_eq!(response["kind"], "cancelled");
    assert_eq!(response["question_id"], "disconnect");
    assert!(
        cluster
            .hub_a
            .lock()
            .await
            .pending_remote_questions
            .is_empty()
    );

    let snapshot = loopal_agent_hub::view_router::handle_snapshot(
        &cluster.hub_a,
        json!({"agent": "hub-b/remote-worker"}),
    )
    .await
    .unwrap();
    assert!(snapshot["state"]["agent"]["conversation"]["pending_question"].is_null());
}

#[tokio::test]
async fn qualified_remote_interrupt_request_is_acknowledged_by_child_agent() {
    let cluster = cluster().await;
    let ui = UiSession::connect(
        cluster.hub_a.clone(),
        "desktop-ui",
        loopal_protocol::UiCapabilities::NONE,
    )
    .await;
    // The mock agent acknowledges every request before forwarding a copy to
    // this receiver, so awaiting the Hub RPC below proves the ACK round trip.
    let (_agent, mut incoming) =
        register_mock_agent(&cluster.hub_b, "remote-worker", Some("hub-a/main")).await;
    ui.client
        .connection()
        .send_request(
            methods::HUB_INTERRUPT.name,
            json!({"target": "hub-b/remote-worker"}),
        )
        .await
        .unwrap();
    let message = tokio::time::timeout(Duration::from_secs(3), incoming.recv())
        .await
        .expect("remote interrupt was not forwarded")
        .expect("remote agent connection closed");
    assert!(matches!(
        message,
        Incoming::Request { method, .. } if method == methods::AGENT_INTERRUPT.name
    ));
}

struct Cluster {
    hub_a: Arc<tokio::sync::Mutex<loopal_agent_hub::Hub>>,
    hub_b: Arc<tokio::sync::Mutex<loopal_agent_hub::Hub>>,
}

async fn cluster() -> Cluster {
    let meta = Arc::new(tokio::sync::Mutex::new(loopal_meta_hub::MetaHub::new()));
    let (hub_a, events_a) = make_hub();
    let (hub_b, events_b) = make_hub();
    let _loop_a = start_event_loop(hub_a.clone(), events_a);
    let _loop_b = start_event_loop(hub_b.clone(), events_b);
    let conn_a = wire_hub_to_meta("hub-a", &hub_a, &meta).await;
    let conn_b = wire_hub_to_meta("hub-b", &hub_b, &meta).await;
    hub_a.lock().await.uplink = Some(Arc::new(HubUplink::new(conn_a, "hub-a".into())));
    hub_b.lock().await.uplink = Some(Arc::new(HubUplink::new(conn_b, "hub-b".into())));
    Cluster { hub_a, hub_b }
}
