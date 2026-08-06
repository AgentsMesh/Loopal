use super::*;

#[tokio::test]
async fn closed_workspace_source_does_not_revoke_live_tcp_lease() {
    let (raw_tx, _raw_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx)));
    let transport = Arc::new(LaggedResyncFailureTransport {
        closed: AtomicBool::new(false),
        first_event: AtomicBool::new(false),
        first_event_started: Notify::new(),
        release_first_event: Notify::new(),
    });
    let (conn, incoming) = Connection::new(transport).into_listening();
    let lease = "workspace-source-lease".to_string();
    start_tcp_ui_io(
        hub.clone(),
        "workspace-source-ui",
        conn,
        incoming,
        UiCapabilities::NONE,
        lease.clone(),
    )
    .await;

    let old_workspace = hub
        .lock()
        .await
        .workspace
        .take()
        .expect("test Hub should install a workspace service");
    drop(old_workspace);
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        hub.lock().await.ui.is_ui_client(&lease),
        "an auxiliary source closing must not kill a live UI connection"
    );
}

#[tokio::test]
async fn blocked_event_send_revokes_capable_tcp_lease() {
    let (raw_tx, _raw_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx)));
    let transport = Arc::new(LaggedResyncFailureTransport {
        closed: AtomicBool::new(false),
        first_event: AtomicBool::new(false),
        first_event_started: Notify::new(),
        release_first_event: Notify::new(),
    });
    let (conn, incoming) = Connection::new(transport.clone()).into_listening();
    let lease = "blocked-send-lease".to_string();
    start_tcp_ui_io(
        hub.clone(),
        "blocked-send-ui",
        conn,
        incoming,
        UiCapabilities {
            plan_approval: true,
            ..UiCapabilities::NONE
        },
        lease.clone(),
    )
    .await;

    hub.lock()
        .await
        .ui
        .event_broadcaster()
        .send(AgentEvent::root(AgentEventPayload::Running))
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while hub.lock().await.ui.is_ui_client(&lease) || !transport.closed.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("blocked UI event send must revoke its capability lease");
}
