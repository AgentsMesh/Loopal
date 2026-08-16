use std::sync::Arc;

use agent_client_protocol_schema::StopReason;
use loopal_agent_hub::{HubClient, UiSession};
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods::VIEW_SNAPSHOT;
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, ROOT_AGENT_NAME};
use tokio::sync::broadcast;

use crate::adapter::AcpAdapter;

fn closed_adapter() -> AcpAdapter {
    let (events, event_rx) = broadcast::channel(1);
    drop(events);
    let (hub_transport, client_transport) = loopal_ipc::duplex_pair();
    let (_hub, _hub_rx) = Connection::new(hub_transport).into_listening();
    let (client, mut client_rx) = Connection::new(client_transport).into_listening();
    tokio::spawn(async move { while client_rx.recv().await.is_some() {} });
    let (writer, _reader) = tokio::io::duplex(64);
    AcpAdapter::new(
        UiSession {
            client: Arc::new(HubClient::new(client)),
            event_rx,
            lease_id: "closed".into(),
        },
        Arc::new(crate::jsonrpc::JsonRpcTransport::with_writer(Box::new(
            writer,
        ))),
    )
}

#[tokio::test]
async fn closed_event_stream_returns_end_turn() {
    assert_eq!(
        closed_adapter().run_event_loop("session-1").await,
        StopReason::EndTurn
    );
}

#[tokio::test]
async fn closed_bootstrap_drain_returns() {
    closed_adapter().drain_bootstrap_events().await;
}

#[tokio::test]
async fn lagged_event_loop_skips_to_terminal_event() {
    let (events, event_rx) = broadcast::channel(1);
    let (hub_transport, client_transport) = loopal_ipc::duplex_pair();
    let (_hub, _hub_rx) = Connection::new(hub_transport).into_listening();
    let (client, mut client_rx) = Connection::new(client_transport).into_listening();
    tokio::spawn(async move { while client_rx.recv().await.is_some() {} });
    let (writer, _reader) = tokio::io::duplex(64);
    let adapter = AcpAdapter::new(
        UiSession {
            client: Arc::new(HubClient::new(client)),
            event_rx,
            lease_id: "lagged".into(),
        },
        Arc::new(crate::jsonrpc::JsonRpcTransport::with_writer(Box::new(
            writer,
        ))),
    );
    events
        .send(AgentEvent::root(AgentEventPayload::Started))
        .unwrap();
    events
        .send(AgentEvent::root(AgentEventPayload::Finished))
        .unwrap();
    assert_eq!(
        adapter.run_event_loop("session-1").await,
        StopReason::EndTurn
    );
}

#[tokio::test]
async fn lagged_event_loop_resyncs_authoritative_root_snapshot() {
    let (events, event_rx) = broadcast::channel(1);
    let (hub_transport, client_transport) = loopal_ipc::duplex_pair();
    let (hub, mut hub_rx) = Connection::new(hub_transport).into_listening();
    let (client, mut client_rx) = Connection::new(client_transport).into_listening();
    tokio::spawn(async move { while client_rx.recv().await.is_some() {} });
    let (writer, reader) = tokio::io::duplex(4096);
    let adapter = Arc::new(AcpAdapter::new(
        UiSession {
            client: Arc::new(HubClient::new(client)),
            event_rx,
            lease_id: "lagged-resync".into(),
        },
        Arc::new(crate::jsonrpc::JsonRpcTransport::with_writer(Box::new(
            writer,
        ))),
    ));
    events
        .send(AgentEvent::root(AgentEventPayload::Started))
        .unwrap();
    events
        .send(AgentEvent::root(AgentEventPayload::Running))
        .unwrap();

    let loop_task = tokio::spawn({
        let adapter = adapter.clone();
        async move { adapter.run_event_loop("session-1").await }
    });
    let Incoming::Request { id, method, params } = hub_rx.recv().await.unwrap() else {
        panic!("expected view/snapshot request after lag");
    };
    assert_eq!(method, VIEW_SNAPSHOT.name);
    assert_eq!(params["agent"], ROOT_AGENT_NAME);
    hub.respond(
        id,
        serde_json::json!({
            "state": {"bg_tasks": {}, "crons": [], "tasks": []}
        }),
    )
    .await
    .unwrap();
    let mut reader = tokio::io::BufReader::new(reader);
    let mut line = String::new();
    tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(line.trim()).unwrap()["method"],
        "_loopal/crons"
    );

    events
        .send(AgentEvent::named(
            QualifiedAddress::local(ROOT_AGENT_NAME),
            AgentEventPayload::Finished,
        ))
        .unwrap();
    assert_eq!(loop_task.await.unwrap(), StopReason::EndTurn);
}

#[tokio::test]
async fn bootstrap_drain_ignores_child_terminal_events() {
    let (events, event_rx) = broadcast::channel(4);
    let (hub_transport, client_transport) = loopal_ipc::duplex_pair();
    let (_hub, _hub_rx) = Connection::new(hub_transport).into_listening();
    let (client, mut client_rx) = Connection::new(client_transport).into_listening();
    tokio::spawn(async move { while client_rx.recv().await.is_some() {} });
    let (writer, _reader) = tokio::io::duplex(64);
    let adapter = AcpAdapter::new(
        UiSession {
            client: Arc::new(HubClient::new(client)),
            event_rx,
            lease_id: "bootstrap-root-only".into(),
        },
        Arc::new(crate::jsonrpc::JsonRpcTransport::with_writer(Box::new(
            writer,
        ))),
    );
    events
        .send(AgentEvent::named(
            QualifiedAddress::local("worker"),
            AgentEventPayload::Finished,
        ))
        .unwrap();
    events
        .send(AgentEvent::root(AgentEventPayload::AwaitingInput))
        .unwrap();
    adapter.drain_bootstrap_events().await;
}
