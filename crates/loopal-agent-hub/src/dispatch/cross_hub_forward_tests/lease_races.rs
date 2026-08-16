#[tokio::test]
async fn failed_remote_rpc_cannot_rollback_a_same_name_new_generation() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let (hub, meta_connection, mut meta_rx, requester) = hub_with_uplink(event_tx).await;
    let (request_seen_tx, request_seen_rx) = tokio::sync::oneshot::channel();
    let (respond_tx, respond_rx) = tokio::sync::oneshot::channel();
    let responder = tokio::spawn(async move {
        let Incoming::Request { id, method, .. } = meta_rx.recv().await.unwrap() else {
            panic!("expected meta/spawn request");
        };
        assert_eq!(method, methods::META_SPAWN.name);
        request_seen_tx.send(()).unwrap();
        respond_rx.await.unwrap();
        meta_connection
            .respond(id, json!({"message": "remote rejected"}))
            .await
            .unwrap();
    });
    let spawn = tokio::spawn({
        let hub = hub.clone();
        async move { forward_cross_hub_spawn(&hub, signed_spawn("remote-worker"), &requester).await }
    });
    request_seen_rx.await.unwrap();

    let replacement_generation = {
        let mut hub = hub.lock().await;
        hub.registry.unregister_connection("remote-worker");
        hub.registry
            .register_shadow(
                "remote-worker",
                loopal_protocol::QualifiedAddress::local("replacement-parent"),
            )
            .unwrap();
        hub.registry.generation("remote-worker").unwrap()
    };
    respond_tx.send(()).unwrap();
    assert!(spawn.await.unwrap().is_err());
    responder.await.unwrap();

    let hub = hub.lock().await;
    assert_eq!(
        hub.registry.generation("remote-worker"),
        Some(replacement_generation)
    );
    assert_eq!(
        hub.registry
            .agent_info("remote-worker")
            .and_then(|info| info.parent.as_ref())
            .map(ToString::to_string)
            .as_deref(),
        Some("replacement-parent")
    );
}

#[tokio::test]
async fn spawn_response_on_superseded_uplink_terminalizes_shadow_fail_closed() {
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let (hub, meta_connection, mut meta_rx, requester) = hub_with_uplink(event_tx).await;
    let (request_seen_tx, request_seen_rx) = tokio::sync::oneshot::channel();
    let (respond_tx, respond_rx) = tokio::sync::oneshot::channel();
    let responder = tokio::spawn(async move {
        let Incoming::Request { id, method, .. } = meta_rx.recv().await.unwrap() else {
            panic!("expected meta/spawn request");
        };
        assert_eq!(method, methods::META_SPAWN.name);
        request_seen_tx.send(()).unwrap();
        respond_rx.await.unwrap();
        meta_connection
            .respond(id, json!({"agent_id": "remote-id-on-old-lease"}))
            .await
            .unwrap();
    });
    let spawn = tokio::spawn({
        let hub = hub.clone();
        async move { forward_cross_hub_spawn(&hub, signed_spawn("lease-race-worker"), &requester).await }
    });
    request_seen_rx.await.unwrap();

    let (replacement_transport, _replacement_peer) = loopal_ipc::duplex_pair();
    let (replacement_connection, _replacement_rx) =
        Connection::new(replacement_transport).into_listening();
    hub.lock().await.uplink = Some(Arc::new(HubUplink::new(
        replacement_connection,
        "origin".into(),
    )));
    respond_tx.send(()).unwrap();

    let error = spawn.await.unwrap().unwrap_err();
    assert!(error.contains("superseded MetaHub uplink lease"));
    responder.await.unwrap();
    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::SubAgentSpawned(ref event)
            if event.name == "lease-race-worker" && event.agent_id == "unknown"
    ));
    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::Error { .. }
    ));
    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::Finished
    ));
    assert_eq!(
        hub.lock()
            .await
            .registry
            .completion("lease-race-worker")
            .map(|completion| completion.reason.as_str()),
        Some("remote_spawn_outcome_unknown")
    );
}
