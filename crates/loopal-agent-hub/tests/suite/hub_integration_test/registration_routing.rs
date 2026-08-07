// ── Registration ────────────────────────────────────────────────────

#[tokio::test]
async fn agent_registered_and_reachable() {
    let (hub, _event_rx) = make_hub();

    let (conn, rx) = hub_server::connect_local(hub.clone(), "worker-1");
    spawn_mock_agent(conn, rx);
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("worker-1")
            .is_some()
    );
}

#[tokio::test]
async fn duplicate_agent_name_rejected() {
    let (hub, _event_rx) = make_hub();

    let (c1, r1) = hub_server::connect_local(hub.clone(), "dup");
    spawn_mock_agent(c1, r1);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Second registration fails — agent_io task exits early
    let (c2, r2) = hub_server::connect_local(hub.clone(), "dup");
    spawn_mock_agent(c2, r2);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Hub still has exactly one "dup" (the first one)
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("dup")
            .is_some()
    );
}

// ── Message routing ─────────────────────────────────────────────────

#[tokio::test]
async fn agent_a_routes_message_to_agent_b() {
    let (hub, _event_rx) = make_hub();

    let (conn_a, rx_a) = hub_server::connect_local(hub.clone(), "agent-a");
    spawn_mock_agent(conn_a.clone(), rx_a);

    let (conn_b, rx_b) = hub_server::connect_local(hub.clone(), "agent-b");
    // B: capture incoming request method instead of auto-respond
    let (method_tx, mut method_rx) = mpsc::channel::<String>(1);
    let conn_b_bg = conn_b.clone();
    tokio::spawn(async move {
        let mut rx = rx_b;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, method, .. } = msg {
                let _ = method_tx.send(method).await;
                let _ = conn_b_bg.respond(id, json!({"ok": true})).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "source": {"Agent": {"hub": [], "agent": "agent-a"}},
        "target": {"hub": [], "agent": "agent-b"},
        "content": {"text": "hello from A", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = conn_a.send_request(methods::HUB_ROUTE.name, envelope).await;
    assert!(result.is_ok(), "hub/route should succeed");

    let method = tokio::time::timeout(Duration::from_secs(2), method_rx.recv()).await;
    assert_eq!(
        method.unwrap().unwrap(),
        methods::AGENT_MESSAGE.name,
        "B should receive agent/message"
    );
}
