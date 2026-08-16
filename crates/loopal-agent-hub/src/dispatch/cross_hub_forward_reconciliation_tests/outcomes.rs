#[tokio::test]
async fn rejected_spawn_reconciles_a_completion_observed_first() {
    let (events, _event_rx) = mpsc::channel::<AgentEvent>(16);
    let (hub, meta, mut meta_rx, requester) = hub_with_uplink(events).await;
    let (seen_tx, seen_rx) = oneshot::channel();
    let (respond_tx, respond_rx) = oneshot::channel();
    let responder = tokio::spawn(async move {
        let Incoming::Request { id, .. } = meta_rx.recv().await.unwrap() else {
            panic!("expected meta/spawn request");
        };
        seen_tx.send(()).unwrap();
        respond_rx.await.unwrap();
        meta.respond(
            id,
            loopal_ipc::cross_hub::RemoteSpawnOutcome::RejectedBeforeSideEffect {
                message: "rejected".into(),
            }
            .into_value(),
        )
        .await
        .unwrap();
    });
    let spawn = tokio::spawn({
        let hub = hub.clone();
        async move { forward_cross_hub_spawn(&hub, signed_spawn("early"), &requester).await }
    });
    seen_rx.await.unwrap();
    let early = cached("early", "main");
    assert!(
        crate::finish::cache_cross_hub_completion_if_spawning(
            &hub,
            "early",
            early.completion,
            early.envelope,
        )
        .await
    );
    respond_tx.send(()).unwrap();

    let error = spawn.await.unwrap().unwrap_err();
    assert!(error.contains("rejected after a completion was observed"));
    responder.await.unwrap();
}

#[tokio::test]
async fn unknown_outcome_reports_cached_reconciliation_and_stale_shadow() {
    let (events, _event_rx) = mpsc::channel::<AgentEvent>(16);
    let (hub, _meta, _meta_rx, requester) = hub_with_uplink(events).await;
    let (sink, generation, uplink) = {
        let mut hub = hub.lock().await;
        let uplink = hub.uplink.clone().unwrap();
        let shadow = hub
            .registry
            .register_shadow_with_parent_policy_execution(
                "cached-unknown",
                QualifiedAddress::local("main"),
                true,
            )
            .unwrap();
        hub.install_shadow_spawn_admission(
            "cached-unknown",
            shadow.connection_generation,
            uplink.clone(),
        );
        (
            AuthoritativeEventSink::from_hub(&hub),
            shadow.connection_generation,
            uplink,
        )
    };
    let early = cached("cached-unknown", "main");
    assert!(
        crate::finish::cache_cross_hub_completion_if_spawning(
            &hub,
            "cached-unknown",
            early.completion,
            early.envelope,
        )
        .await
    );
    assert_eq!(
        resolve_unknown_outcome(
            &hub,
            &sink,
            "cached-unknown",
            generation,
            &uplink,
            "main",
            Some(requester.connection_generation),
            None,
            "unknown",
        )
        .await,
        "cached remote completion reconciled"
    );
    assert_eq!(
        resolve_unknown_outcome(
            &hub,
            &sink,
            "missing-shadow",
            generation,
            &uplink,
            "main",
            Some(requester.connection_generation),
            None,
            "unknown",
        )
        .await,
        "stale local shadow quarantined"
    );
}
