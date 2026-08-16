use super::*;

fn connection(transport: Arc<ForwardingTransport>) -> Arc<Connection<loopal_ipc::Listening>> {
    Connection::new(transport).into_listening().0
}

#[tokio::test]
async fn workspace_notification_send_failure_ends_only_its_forwarder() {
    let (events, receiver) = tokio::sync::broadcast::channel(2);
    let transport = ForwardingTransport::new(true);
    events
        .send(loopal_workspace::ServiceNotification {
            method: "workspace/test",
            params: serde_json::json!({"revision": 1}),
        })
        .unwrap();

    super::super::forward::forward_service_events(
        "workspace-ui".into(),
        receiver,
        connection(transport.clone()),
    )
    .await;

    let frames = transport.frames();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["method"], "workspace/test");
}

#[tokio::test]
async fn lagged_workspace_stream_sends_a_bounded_resync_notice() {
    let (events, receiver) = tokio::sync::broadcast::channel(1);
    for revision in 1..=3 {
        events
            .send(loopal_workspace::ServiceNotification {
                method: "workspace/test",
                params: serde_json::json!({"revision": revision}),
            })
            .unwrap();
    }
    let transport = ForwardingTransport::new(true);

    super::super::forward::forward_service_events(
        "lagged-workspace-ui".into(),
        receiver,
        connection(transport.clone()),
    )
    .await;

    let frames = transport.frames();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["method"], methods::WORKSPACE_RESYNC_REQUIRED.name);
    assert_eq!(frames[0]["params"]["reason"], "event_lag");
    assert_eq!(frames[0]["params"]["droppedEvents"], 2);
}

#[tokio::test]
async fn each_closed_agent_event_source_ends_the_forwarder() {
    let (events, event_rx) = tokio::sync::broadcast::channel(2);
    let (_resync, resync_rx) = tokio::sync::broadcast::channel(2);
    drop(events);
    super::super::forward::forward_events(
        "closed-events".into(),
        event_rx,
        resync_rx,
        connection(ForwardingTransport::new(false)),
    )
    .await;

    let (events, event_rx) = tokio::sync::broadcast::channel(2);
    let (resync, resync_rx) = tokio::sync::broadcast::channel(2);
    drop(resync);
    super::super::forward::forward_events(
        "closed-resync".into(),
        event_rx,
        resync_rx,
        connection(ForwardingTransport::new(false)),
    )
    .await;
    drop(events);
}

#[tokio::test]
async fn direct_and_lagged_resync_failures_end_the_forwarder() {
    let (events, event_rx) = tokio::sync::broadcast::channel(2);
    let (resync, resync_rx) = tokio::sync::broadcast::channel(2);
    let transport = ForwardingTransport::new(true);
    resync.send(()).unwrap();
    super::super::forward::forward_events(
        "direct-resync".into(),
        event_rx,
        resync_rx,
        connection(transport.clone()),
    )
    .await;
    assert_eq!(
        transport.frames()[0]["method"],
        methods::VIEW_RESYNC_REQUIRED.name
    );
    drop(events);

    let (events, event_rx) = tokio::sync::broadcast::channel(2);
    let (resync, resync_rx) = tokio::sync::broadcast::channel(1);
    for _ in 0..3 {
        resync.send(()).unwrap();
    }
    let transport = ForwardingTransport::new(true);
    super::super::forward::forward_events(
        "lagged-resync".into(),
        event_rx,
        resync_rx,
        connection(transport.clone()),
    )
    .await;
    assert_eq!(
        transport.frames()[0]["method"],
        methods::VIEW_RESYNC_REQUIRED.name
    );
    drop(events);
}

#[tokio::test]
async fn agent_event_is_serialized_before_the_source_closes() {
    let (events, event_rx) = tokio::sync::broadcast::channel(2);
    let (_resync, resync_rx) = tokio::sync::broadcast::channel(2);
    let transport = ForwardingTransport::new(false);
    events
        .send(AgentEvent::root(AgentEventPayload::Running))
        .unwrap();
    drop(events);

    super::super::forward::forward_events(
        "event-ui".into(),
        event_rx,
        resync_rx,
        connection(transport.clone()),
    )
    .await;

    assert_eq!(transport.frames().len(), 1);
    assert_eq!(transport.frames()[0]["method"], methods::AGENT_EVENT.name);
}
