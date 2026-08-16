// ── Control + Interrupt ─────────────────────────────────────────────

#[tokio::test]
async fn hub_control_reaches_target_agent() {
    let (hub, _event_rx) = make_hub();

    let ui = connect_control_ui(hub.clone()).await;
    let sender = ui.client.connection().clone();

    // Target: capture method of incoming request
    let (target_conn, target_rx) = hub_server::connect_local(hub.clone(), "target");
    let (method_tx, mut method_rx) = mpsc::channel::<String>(1);
    let tc = target_conn.clone();
    tokio::spawn(async move {
        let mut rx = target_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, method, .. } = msg {
                let _ = method_tx.send(method).await;
                let _ = tc.respond(id, json!({"status": "applied"})).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = sender
        .send_request(
            methods::HUB_CONTROL.name,
            json!({"target": "target", "command": {"Clear": null}}),
        )
        .await;
    assert_eq!(result.unwrap(), json!({"status": "applied"}));

    let method = tokio::time::timeout(Duration::from_secs(2), method_rx.recv()).await;
    assert_eq!(method.unwrap().unwrap(), methods::AGENT_CONTROL.name);
}

#[tokio::test]
async fn hub_control_normalizes_legacy_queue_acknowledgement() {
    let (hub, _event_rx) = make_hub();
    let ui = connect_control_ui(hub.clone()).await;
    let sender = ui.client.connection().clone();

    let (target_conn, mut target_rx) = hub_server::connect_local(hub, "legacy-target");
    tokio::spawn(async move {
        while let Some(message) = target_rx.recv().await {
            if let Incoming::Request { id, method, .. } = message
                && method == methods::AGENT_CONTROL.name
            {
                target_conn
                    .respond(id, json!({"ok": true}))
                    .await
                    .expect("send legacy queue acknowledgement");
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let response = sender
        .send_request(
            methods::HUB_CONTROL.name,
            json!({"target": "legacy-target", "command": {"Clear": null}}),
        )
        .await
        .expect("legacy acknowledgement must remain compatible");
    assert_eq!(response, json!({"status": "queued"}));
}

#[tokio::test(start_paused = true)]
async fn hub_control_timeout_preserves_connection_and_late_application() {
    let (hub, _event_rx) = make_hub();

    let ui = connect_control_ui(hub.clone()).await;
    let sender = ui.client.connection().clone();

    let (target_conn, mut target_rx) = hub_server::connect_local(hub, "slow-target");
    let target_for_responses = target_conn.clone();
    let (late_application_tx, mut late_application_rx) = mpsc::channel::<()>(1);
    let cancellation_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancellation_seen_by_agent = Arc::clone(&cancellation_seen);
    tokio::spawn(async move {
        let mut controls = 0;
        while let Some(message) = target_rx.recv().await {
            match message {
                Incoming::Request { id, method, .. } if method == methods::AGENT_CONTROL.name => {
                    controls += 1;
                    if controls == 1 {
                        tokio::time::sleep(
                            loopal_protocol::DEFAULT_CONTROL_RPC_TIMEOUT + Duration::from_secs(1),
                        )
                        .await;
                        late_application_tx
                            .send(())
                            .await
                            .expect("observe late control application");
                    }
                    target_for_responses
                        .respond(id, json!({"status": "applied"}))
                        .await
                        .expect("respond to control request");
                }
                Incoming::Notification { method, .. } if method == methods::REQUEST_CANCEL.name => {
                    cancellation_seen_by_agent.store(true, std::sync::atomic::Ordering::Release);
                }
                _ => {}
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let timed_out = sender
        .send_request(
            methods::HUB_CONTROL.name,
            json!({"target": "slow-target", "command": {"Clear": null}}),
        )
        .await
        .expect("Hub timeout is an unknown acknowledgement, not an RPC failure");
    assert_eq!(timed_out, json!({"status": "unknown"}));

    tokio::time::timeout(Duration::from_secs(2), late_application_rx.recv())
        .await
        .expect("late control must still be applied")
        .expect("late application signal channel closed");
    assert!(
        target_conn.is_connected(),
        "an unknown control outcome must not close the agent connection"
    );

    let follow_up = sender
        .send_request(
            methods::HUB_CONTROL.name,
            json!({"target": "slow-target", "command": {"Clear": null}}),
        )
        .await
        .expect("the preserved agent connection must accept later controls");
    assert_eq!(follow_up, json!({"status": "applied"}));
    assert!(
        !cancellation_seen.load(std::sync::atomic::Ordering::Acquire),
        "Hub must retain the response waiter instead of cancelling accepted control work"
    );
}

#[tokio::test]
async fn hub_interrupt_reaches_target_agent() {
    let (hub, _event_rx) = make_hub();

    let ui = connect_control_ui(hub.clone()).await;
    let sender = ui.client.connection().clone();

    // Target: acknowledge only after the interrupt request is observed.
    let (target_conn, target_rx) = hub_server::connect_local(hub.clone(), "target");
    let (method_tx, mut method_rx) = mpsc::channel::<String>(1);
    tokio::spawn(async move {
        let mut rx = target_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, method, .. } = msg {
                let _ = target_conn.respond(id, json!({"ok": true})).await;
                let _ = method_tx.send(method).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = sender
        .send_request(methods::HUB_INTERRUPT.name, json!({"target": "target"}))
        .await;
    assert!(result.is_ok());

    let method = tokio::time::timeout(Duration::from_secs(2), method_rx.recv()).await;
    assert_eq!(method.unwrap().unwrap(), methods::AGENT_INTERRUPT.name);
}

// ── Error handling ──────────────────────────────────────────────────

#[tokio::test]
async fn hub_interrupt_reports_closed_target_transport() {
    let (hub, _event_rx) = make_hub();
    let ui = connect_control_ui(hub.clone()).await;
    let sender = ui.client.connection().clone();
    let (transport, _peer) = loopal_ipc::duplex_pair();
    let (target, _rx) = Connection::new(transport).into_listening();
    hub.lock()
        .await
        .registry
        .register_connection("closed-target", target.clone())
        .unwrap();
    target.close().await;

    let error = sender
        .send_request(
            methods::HUB_INTERRUPT.name,
            json!({"target": "closed-target"}),
        )
        .await
        .expect_err("closed transport must not report a successful interrupt");
    assert!(matches!(error, RpcError::Remote { .. }), "got: {error:?}");
    assert!(
        error
            .to_string()
            .contains("interrupt to 'closed-target' failed")
    );
}

#[tokio::test]
async fn malformed_route_does_not_crash() {
    let (hub, _event_rx) = make_hub();
    let (conn, rx) = hub_server::connect_local(hub.clone(), "sender");
    spawn_mock_agent(conn.clone(), rx);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Invalid envelope JSON — should return Err (not crash / not transport error)
    let result = conn
        .send_request(methods::HUB_ROUTE.name, json!({"garbage": true}))
        .await;
    let err = result.expect_err("malformed envelope should surface as Err");
    assert!(matches!(err, RpcError::Remote { .. }), "got: {err:?}");
}

#[tokio::test]
async fn missing_required_field_returns_error() {
    let (hub, _event_rx) = make_hub();
    let (conn, rx) = hub_server::connect_local(hub.clone(), "sender");
    spawn_mock_agent(conn.clone(), rx);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // hub/agent_info without name field → should return Err
    let result = conn
        .send_request(methods::HUB_AGENT_INFO.name, json!({}))
        .await;
    let err = result.expect_err("missing-field should surface as Err, not transport failure");
    assert!(matches!(err, RpcError::Remote { .. }), "got: {err:?}");
}

// ── Event propagation ───────────────────────────────────────────────

#[tokio::test]
async fn agent_event_reaches_hub_event_channel() {
    let (hub, mut event_rx) = make_hub();

    let (agent_conn, agent_rx) = hub_server::connect_local(hub.clone(), "worker");
    spawn_mock_agent(agent_conn.clone(), agent_rx);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Agent sends event notification
    let event = json!({
        "agent_name": null,
        "payload": {"Stream": {"text": "hello"}}
    });
    agent_conn
        .send_notification(methods::AGENT_EVENT.name, event)
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_secs(2), event_rx.recv()).await;
    assert!(received.is_ok(), "Hub should forward event");
    let evt = received.unwrap().unwrap();
    assert_eq!(
        evt.agent_name.as_ref().map(|a| a.to_string()).as_deref(),
        Some("worker")
    );
}
