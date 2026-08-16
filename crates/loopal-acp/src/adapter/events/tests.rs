use std::sync::Arc;

use agent_client_protocol_schema::StopReason;
use loopal_agent_hub::{HubClient, UiSession};
use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::{
    AgentEvent, AgentEventPayload, PermissionIntent, PermissionIntentRequest, QualifiedAddress,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;

use super::*;

#[path = "tests/lifecycle.rs"]
mod lifecycle;
#[path = "tests/routing.rs"]
mod routing;

struct EventHarness {
    adapter: Arc<AcpAdapter>,
    events: broadcast::Sender<AgentEvent>,
    acp_reader: BufReader<tokio::io::DuplexStream>,
    hub: Arc<Connection<loopal_ipc::connection::Listening>>,
    hub_rx: tokio::sync::mpsc::Receiver<Incoming>,
}

fn harness() -> EventHarness {
    let (hub_transport, client_transport) = loopal_ipc::duplex_pair();
    let (hub, hub_rx) = Connection::new(hub_transport).into_listening();
    let (client, mut client_rx) = Connection::new(client_transport).into_listening();
    tokio::spawn(async move { while client_rx.recv().await.is_some() {} });
    let (events, event_rx) = broadcast::channel(16);
    let ui = UiSession {
        client: Arc::new(HubClient::new(client)),
        event_rx,
        lease_id: "test-ui".into(),
    };
    let (acp_writer, acp_reader) = tokio::io::duplex(4096);
    let adapter = Arc::new(AcpAdapter::new(
        ui,
        Arc::new(crate::jsonrpc::JsonRpcTransport::with_writer(Box::new(
            acp_writer,
        ))),
    ));
    EventHarness {
        adapter,
        events,
        acp_reader: BufReader::new(acp_reader),
        hub,
        hub_rx,
    }
}

fn permission_intent() -> PermissionIntent {
    let request = PermissionIntentRequest::create(
        "tool-1",
        "Bash",
        json!({"command":"pwd"}),
        json!({"command":"pwd"}),
        json!({"type":"object"}),
        None,
    )
    .unwrap();
    PermissionIntent::bind(request.intent_seed, 1, 1, "opaque-token").unwrap()
}

async fn read_request(reader: &mut BufReader<tokio::io::DuplexStream>) -> Value {
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

#[tokio::test]
async fn permission_event_forwards_digest_and_loop_continues() {
    let mut harness = harness();
    let intent = permission_intent();
    let digest = intent.intent_digest();
    let loop_task = tokio::spawn({
        let adapter = harness.adapter.clone();
        async move { adapter.run_event_loop("session-1").await }
    });
    harness
        .events
        .send(AgentEvent::named(
            QualifiedAddress::local("worker"),
            AgentEventPayload::ToolPermissionRequest {
                id: "permission-1".into(),
                name: "Bash".into(),
                input: json!({"command":"pwd"}),
                permission_intent: Some(Box::new(intent)),
            },
        ))
        .unwrap();
    let request = read_request(&mut harness.acp_reader).await;
    harness
        .adapter
        .acp_out
        .route_response(
            request["id"].as_i64().unwrap(),
            json!({"outcome":{"outcome":"selected","optionId":"allow_once"}}),
        )
        .await;
    let Incoming::Request { id, params, .. } = harness.hub_rx.recv().await.unwrap() else {
        panic!("expected permission response");
    };
    assert_eq!(params["agent_name"], "worker");
    assert_eq!(params["permission_intent_digest"], json!(digest));
    harness.hub.respond(id, json!({})).await.unwrap();
    harness
        .events
        .send(AgentEvent::root(AgentEventPayload::AwaitingInput))
        .unwrap();
    assert_eq!(loop_task.await.unwrap(), StopReason::EndTurn);
}

#[tokio::test]
async fn permission_event_defaults_agent_and_omits_digest() {
    let mut harness = harness();
    let loop_task = tokio::spawn({
        let adapter = harness.adapter.clone();
        async move { adapter.run_event_loop("session-1").await }
    });
    harness
        .events
        .send(AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
            id: "permission-2".into(),
            name: "Read".into(),
            input: json!({"file_path":"a"}),
            permission_intent: None,
        }))
        .unwrap();
    let request = read_request(&mut harness.acp_reader).await;
    harness
        .adapter
        .acp_out
        .route_response(request["id"].as_i64().unwrap(), json!({"outcome":"deny"}))
        .await;
    let Incoming::Request { id, params, .. } = harness.hub_rx.recv().await.unwrap() else {
        panic!("expected permission response");
    };
    assert_eq!(params["agent_name"], "main");
    assert!(params["permission_intent_digest"].is_null());
    harness.hub.respond(id, json!({})).await.unwrap();
    harness
        .events
        .send(AgentEvent::root(AgentEventPayload::Finished))
        .unwrap();
    assert_eq!(loop_task.await.unwrap(), StopReason::EndTurn);
}
