#[tokio::test]
async fn awaiting_input_does_not_stop_a_workflow_aware_server() {
    let (client, _server, _incoming) = client_pair();
    let (event_tx, event_rx) = tokio::sync::broadcast::channel(8);
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Stream {
            text: "workflow started; ".into(),
        }))
        .unwrap();
    event_tx
        .send(AgentEvent::root(AgentEventPayload::AwaitingInput))
        .unwrap();
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Stream {
            text: "workflow finished".into(),
        }))
        .unwrap();
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Finished))
        .unwrap();

    assert_eq!(
        consume_events(event_rx, client).await,
        "workflow started; workflow finished"
    );
}

#[tokio::test]
async fn ignores_awaiting_input_before_stream_and_stops_on_error() {
    let (client, _server, _incoming) = client_pair();
    let (event_tx, event_rx) = tokio::sync::broadcast::channel(4);
    event_tx
        .send(AgentEvent::root(AgentEventPayload::AwaitingInput))
        .unwrap();
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Started))
        .unwrap();
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Error {
            message: "failed".into(),
        }))
        .unwrap();
    assert!(consume_events(event_rx, client).await.is_empty());
}

#[tokio::test]
async fn closed_and_lagged_receivers_finish_without_output() {
    let (client, _server, _incoming) = client_pair();
    let (event_tx, event_rx) = tokio::sync::broadcast::channel(2);
    drop(event_tx);
    assert!(consume_events(event_rx, client).await.is_empty());

    let (client, _server, _incoming) = client_pair();
    let (event_tx, event_rx) = tokio::sync::broadcast::channel(2);
    for payload in [
        AgentEventPayload::Started,
        AgentEventPayload::Running,
        AgentEventPayload::Started,
        AgentEventPayload::Finished,
    ] {
        event_tx.send(AgentEvent::root(payload)).unwrap();
    }
    assert!(consume_events(event_rx, client).await.is_empty());
}
