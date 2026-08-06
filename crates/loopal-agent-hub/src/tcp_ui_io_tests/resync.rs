use super::*;

#[tokio::test]
async fn failed_resync_unregisters_capable_tcp_lease() {
    let (raw_tx, _raw_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx)));
    let transport = Arc::new(LaggedResyncFailureTransport {
        closed: AtomicBool::new(false),
        first_event: AtomicBool::new(false),
        first_event_started: Notify::new(),
        release_first_event: Notify::new(),
    });
    let (conn, incoming) = Connection::new(transport.clone()).into_listening();
    let lease = "lagged-lease".to_string();
    start_tcp_ui_io(
        hub.clone(),
        "lagged-ui",
        conn,
        incoming,
        UiCapabilities {
            plan_approval: true,
            ..UiCapabilities::NONE
        },
        lease.clone(),
    )
    .await;
    assert!(
        hub.lock()
            .await
            .ui
            .client_has_capability(&lease, UiCapability::PlanApproval)
    );

    let events = hub.lock().await.ui.event_broadcaster();
    events
        .send(AgentEvent::root(AgentEventPayload::Running))
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(1),
        transport.first_event_started.notified(),
    )
    .await
    .unwrap();
    for _ in 0..300 {
        events
            .send(AgentEvent::root(AgentEventPayload::Running))
            .unwrap();
    }
    transport.release_first_event.notify_one();

    tokio::time::timeout(Duration::from_secs(1), async {
        while hub.lock().await.ui.is_ui_client(&lease) || !transport.closed.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("failed resync must end the supervisor and revoke capability");
    assert!(
        transport.closed.load(Ordering::SeqCst),
        "failed supervisor path must close the client transport"
    );
}
