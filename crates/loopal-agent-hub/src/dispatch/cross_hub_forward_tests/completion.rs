#[tokio::test]
async fn remote_completion_before_spawn_response_is_cached_without_blocking_and_drained_in_order() {
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let (hub, meta_connection, mut meta_rx, requester) = hub_with_uplink(event_tx).await;
    let completion_lease = hub.lock().await.uplink.clone().unwrap();
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
            .respond(id, json!({"agent_id": "instant-remote-id"}))
            .await
            .unwrap();
    });
    let spawn = tokio::spawn({
        let hub = hub.clone();
        let spawn_requester = requester.clone();
        async move {
            forward_cross_hub_spawn(&hub, signed_spawn("instant-remote"), &spawn_requester).await
        }
    });
    request_seen_rx.await.unwrap();
    let typed_completion =
        loopal_protocol::AgentCompletion::new("error", Some("failed immediately".into()));
    let envelope = loopal_protocol::Envelope::new(
        loopal_protocol::MessageSource::AgentResult {
            child: loopal_protocol::QualifiedAddress::local("instant-remote"),
        },
        loopal_protocol::QualifiedAddress::local("main"),
        "failed immediately",
    )
    .with_agent_completion(typed_completion.clone());
    let cached = tokio::time::timeout(
        Duration::from_millis(100),
        crate::finish::cache_cross_hub_completion_if_spawning(
            &hub,
            "instant-remote",
            typed_completion,
            envelope,
        ),
    )
    .await
    .expect("reverse completion admission must not wait for spawn RPC response");
    assert!(cached);

    respond_tx.send(()).unwrap();
    assert_eq!(
        spawn.await.unwrap().unwrap()["agent_id"],
        "instant-remote-id"
    );
    responder.await.unwrap();

    let spawned = event_rx.recv().await.unwrap();
    assert!(matches!(
        spawned.payload,
        AgentEventPayload::SubAgentSpawned(ref event)
            if event.name == "instant-remote"
    ));
    let error = event_rx.recv().await.unwrap();
    assert!(matches!(error.payload, AgentEventPayload::Error { .. }));
    let finished = event_rx.recv().await.unwrap();
    assert!(matches!(finished.payload, AgentEventPayload::Finished));

    let reuse_error = forward_cross_hub_spawn(&hub, signed_spawn("instant-remote"), &requester)
        .await
        .unwrap_err();
    assert!(reuse_error.contains("quarantined"));

    // A non-cross-hub registration path cannot make an old duplicate
    // completion authoritative for the replacement generation either.
    let replacement_generation = {
        let mut h = hub.lock().await;
        h.registry
            .register_shadow(
                "instant-remote",
                loopal_protocol::QualifiedAddress::local("replacement-parent"),
            )
            .unwrap();
        h.registry.generation("instant-remote").unwrap()
    };
    assert!(matches!(
        crate::finish::record_cross_hub_completion_from_uplink(
            &hub,
            "instant-remote",
            loopal_protocol::AgentCompletion::goal(Some("late duplicate".into())),
            completion_lease.connection(),
        )
        .await,
        crate::finish::CrossHubCompletionRoute::Consumed
    ));
    let h = hub.lock().await;
    assert_eq!(
        h.registry.generation("instant-remote"),
        Some(replacement_generation)
    );
    assert!(h.registry.completion("instant-remote").is_none());
}
