#[tokio::test]
async fn worker_handshake_handler_accepts_only_exact_bound_authority() {
    let mut fixture = worker_fixture(true, true).await;
    let (owner, _) = worker_authority(&fixture.hub, &fixture.principal, &fixture.request)
        .await
        .unwrap();
    assert_eq!(owner.session_id, "session-worker");

    let dispatcher = crate::dispatch::build_hub_dispatcher(fixture.hub.clone());
    let error = crate::dispatch::dispatch_hub_request_with_principal(
        &fixture.hub,
        &dispatcher,
        methods::HUB_WORKFLOW_WORKER_HANDSHAKE.name,
        serde_json::to_value(&fixture.request).unwrap(),
        Arc::new(HubRequestPrincipal::Agent(fixture.principal.clone())),
    )
    .await
    .unwrap_err();
    assert!(error.contains("owner is invalid"), "{error}");

    let variants = [
        {
            let mut facts = fixture.facts.clone();
            facts.origin = AgentOrigin::ManagedRoot;
            facts
        },
        {
            let mut facts = fixture.facts.clone();
            facts.depth = 0;
            facts
        },
        {
            let mut facts = fixture.facts.clone();
            facts.parent = None;
            facts
        },
        {
            let mut facts = fixture.facts.clone();
            facts.root.clear();
            facts
        },
        {
            let mut facts = fixture.facts.clone();
            facts.workflow_permission_causation = Some(causation("forged"));
            facts
        },
        {
            let mut facts = fixture.facts.clone();
            facts.workflow_attempt_capability_digest = None;
            facts
        },
    ];
    for facts in variants {
        assert!(
            fixture
                .hub
                .lock()
                .await
                .registry
                .set_runtime_facts(&fixture.worker, facts.clone())
        );
        let principal = AgentPrincipal::new(fixture.worker.clone(), facts);
        let error = worker_authority(&fixture.hub, &principal, &fixture.request)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("authority is invalid"));
    }

    stop_worker_fixture(&mut fixture).await;
}
