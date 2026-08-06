use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_input::AgentInput;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

use super::{ForwardResult, forward_loop, route_request};
use crate::session_hub::SharedSession;
use crate::session_start::SessionHandle;

#[tokio::test]
async fn interrupt_request_ack_is_sent_only_after_session_is_signaled() {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (server, mut server_rx) = Connection::new(server_transport).into_listening();
    let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(1);
    let interrupt = InterruptSignal::new();
    let (interrupt_tx, mut interrupt_rx) = watch::channel(0u64);
    let session = SharedSession::placeholder(input_tx, interrupt.clone(), Arc::new(interrupt_tx));

    let request = tokio::spawn(async move {
        client
            .send_request(methods::AGENT_INTERRUPT.name, serde_json::json!({}))
            .await
    });
    let Incoming::Request { id, method, params } = server_rx.recv().await.unwrap() else {
        panic!("expected interrupt request");
    };
    route_request(id, &method, params, &session, &server).await;

    assert!(interrupt.is_signaled());
    tokio::time::timeout(Duration::from_secs(1), interrupt_rx.changed())
        .await
        .expect("interrupt watch must change before acknowledgement")
        .unwrap();
    let response = request.await.unwrap().unwrap();
    assert_eq!(response["ok"], true);
}

#[tokio::test]
async fn shutdown_request_ack_is_sent_after_signal_and_exits_forward_loop() {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (server, mut server_rx) = Connection::new(server_transport).into_listening();
    let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(1);
    let interrupt = InterruptSignal::new();
    let (interrupt_tx, mut interrupt_rx) = watch::channel(0u64);
    let session = Arc::new(SharedSession::placeholder(
        input_tx,
        interrupt.clone(),
        Arc::new(interrupt_tx),
    ));
    let agent_task = tokio::spawn(std::future::pending::<Option<loopal_error::AgentOutput>>());
    let shutdown = CancellationToken::new();
    let mut handle = SessionHandle {
        session_id: "active".into(),
        session,
        agent_task,
        lifecycle: loopal_runtime::LifecycleMode::Persistent,
        shutdown: shutdown.clone(),
    };

    let interrupt_at_ack = interrupt.clone();
    let shutdown_at_ack = shutdown.clone();
    let request = tokio::spawn(async move {
        let response = client
            .send_request(methods::AGENT_SHUTDOWN.name, serde_json::json!({}))
            .await;
        assert!(
            interrupt_at_ack.is_signaled(),
            "shutdown ACK became observable before the termination signal"
        );
        assert!(
            shutdown_at_ack.is_cancelled(),
            "shutdown ACK became observable before session termination"
        );
        response
    });
    let outcome = tokio::time::timeout(
        Duration::from_secs(1),
        forward_loop(&mut server_rx, &server, &mut handle),
    )
    .await
    .expect("forward loop must exit after shutdown");

    assert!(matches!(outcome, ForwardResult::Shutdown));
    assert!(interrupt.is_signaled());
    assert!(shutdown.is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), interrupt_rx.changed())
        .await
        .expect("shutdown signal must precede acknowledgement")
        .unwrap();
    let response = request.await.unwrap().unwrap();
    assert_eq!(response["ok"], true);
    handle.agent_task.abort();
}
