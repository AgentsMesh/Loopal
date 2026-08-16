#[tokio::test]
async fn stale_requester_cannot_admit_remote_shadow() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let (hub, _meta_connection, mut meta_rx, requester) = hub_with_uplink(event_tx).await;
    {
        let mut locked = hub.lock().await;
        assert!(locked.registry.unregister_exact(&requester));
        let (_peer, transport) = loopal_ipc::duplex_pair();
        let (replacement, _incoming) = Connection::new(transport).into_listening();
        locked
            .registry
            .register_connection("main", replacement)
            .unwrap();
    }

    let error = forward_cross_hub_spawn(&hub, signed_spawn("worker"), &requester)
        .await
        .unwrap_err();

    assert!(error.contains("stale"));
    assert!(hub.lock().await.registry.agent_info("worker").is_none());
    assert!(
        tokio::time::timeout(Duration::from_millis(20), meta_rx.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn full_queue_backpressures_cross_hub_spawn_without_holding_hub_lock() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Running))
        .await
        .unwrap();
    let (hub, meta_connection, meta_rx, requester) = hub_with_uplink(event_tx).await;
    let responder = respond_to_spawn(meta_connection, meta_rx);
    let spawn = tokio::spawn({
        let hub = hub.clone();
        async move { forward_cross_hub_spawn(&hub, signed_spawn("remote-worker"), &requester).await }
    });
    responder.await.unwrap();
    tokio::task::yield_now().await;
    assert!(
        !spawn.is_finished(),
        "remote spawn must wait for SubAgentSpawned queue capacity"
    );
    assert!(
        hub.lock()
            .await
            .registry
            .agent_info("remote-worker")
            .is_some()
    );
    let guard = tokio::time::timeout(Duration::from_millis(100), hub.lock())
        .await
        .expect("cross-hub event backpressure must not hold the Hub lock");
    drop(guard);

    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::Running
    ));
    let event = event_rx.recv().await.unwrap();
    assert!(matches!(
        event.payload,
        AgentEventPayload::SubAgentSpawned(ref spawned)
            if spawned.name == "remote-worker" && spawned.agent_id == "remote-id"
    ));
    assert_eq!(spawn.await.unwrap().unwrap()["agent_id"], "remote-id");
}

#[tokio::test]
async fn closed_queue_reports_failure_but_preserves_remote_completion_shadow() {
    let (event_tx, event_rx) = mpsc::channel(1);
    drop(event_rx);
    let (hub, meta_connection, meta_rx, requester) = hub_with_uplink(event_tx).await;
    let shutdown = hub.lock().await.shutdown_signal.clone();
    let responder = respond_to_spawn(meta_connection, meta_rx);

    let error = forward_cross_hub_spawn(&hub, signed_spawn("remote-worker"), &requester)
        .await
        .unwrap_err();
    responder.await.unwrap();
    assert!(error.contains("authoritative Hub event queue closed"));
    assert!(
        hub.lock()
            .await
            .registry
            .agent_info("remote-worker")
            .is_some(),
        "remote child exists, so its shadow must remain for late completion routing"
    );
    tokio::time::timeout(Duration::from_millis(100), shutdown.notified())
        .await
        .expect("closed authoritative queue must invalidate the Hub");
}
