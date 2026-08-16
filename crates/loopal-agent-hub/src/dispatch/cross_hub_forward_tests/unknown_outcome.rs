#[tokio::test]
async fn remote_spawn_timeout_terminalizes_in_order_and_quarantines_same_lease_name() {
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let (hub, meta_connection, mut meta_rx, requester) = hub_with_uplink(event_tx).await;
    let old_uplink = hub.lock().await.uplink.clone().unwrap();
    let remote = tokio::spawn(async move {
        let Incoming::Request { method, .. } = meta_rx.recv().await.unwrap() else {
            panic!("expected meta/spawn request");
        };
        assert_eq!(method, methods::META_SPAWN.name);
        tokio::time::sleep(Duration::from_millis(150)).await;
        drop(meta_connection);
    });

    let error = forward_cross_hub_spawn(&hub, signed_spawn("unknown-outcome-worker"), &requester)
        .await
        .unwrap_err();
    assert!(error.contains("outcome unknown"));
    let spawned = event_rx.recv().await.unwrap();
    let terminal_error = event_rx.recv().await.unwrap();
    let finished = event_rx.recv().await.unwrap();
    assert!(matches!(
        spawned.payload,
        AgentEventPayload::SubAgentSpawned(ref event)
            if event.name == "unknown-outcome-worker" && event.agent_id == "unknown"
    ));
    assert!(matches!(
        terminal_error.payload,
        AgentEventPayload::Error { ref message }
            if message.contains("remote spawn outcome unknown")
    ));
    assert!(matches!(finished.payload, AgentEventPayload::Finished));
    assert_eq!(
        hub.lock()
            .await
            .registry
            .completion("unknown-outcome-worker")
            .map(|completion| completion.reason.as_str()),
        Some("remote_spawn_outcome_unknown")
    );

    let quarantined =
        forward_cross_hub_spawn(&hub, signed_spawn("unknown-outcome-worker"), &requester)
            .await
            .unwrap_err();
    assert!(quarantined.contains("quarantined"));

    // Even if another registration path reuses the bare name, a late
    // completion on the indeterminate lease cannot finish it.
    let replacement_generation = {
        let mut h = hub.lock().await;
        h.registry
            .register_shadow(
                "unknown-outcome-worker",
                loopal_protocol::QualifiedAddress::local("replacement-parent"),
            )
            .unwrap();
        h.registry.generation("unknown-outcome-worker").unwrap()
    };
    assert!(matches!(
        crate::finish::record_cross_hub_completion_from_uplink(
            &hub,
            "unknown-outcome-worker",
            loopal_protocol::AgentCompletion::goal(Some("late old result".into())),
            old_uplink.connection(),
        )
        .await,
        crate::finish::CrossHubCompletionRoute::Consumed
    ));
    assert_eq!(
        hub.lock()
            .await
            .registry
            .generation("unknown-outcome-worker"),
        Some(replacement_generation)
    );
    assert!(
        hub.lock()
            .await
            .registry
            .completion("unknown-outcome-worker")
            .is_none()
    );

    let (new_hub_transport, _new_meta_transport) = loopal_ipc::duplex_pair();
    let (new_connection, _new_rx) = Connection::new(new_hub_transport).into_listening();
    let new_uplink = Arc::new(HubUplink::new(new_connection, "origin".into()));
    {
        let mut h = hub.lock().await;
        h.uplink = Some(new_uplink.clone());
        assert!(
            !h.shadow_name_is_quarantined("unknown-outcome-worker", &new_uplink),
            "a new authenticated uplink lease releases the conservative name quarantine"
        );
    }
    remote.await.unwrap();
}
