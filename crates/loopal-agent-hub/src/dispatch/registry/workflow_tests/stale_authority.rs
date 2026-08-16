#[tokio::test]
async fn worker_handshake_reports_each_stale_authority_boundary() {
    let missing = worker_fixture(true, false).await;
    assert!(
        worker_authority(&missing.hub, &missing.principal, &missing.request)
            .await
            .unwrap_err()
            .to_string()
            .contains("missing workflow worker runtime authority")
    );

    let mut stale = missing.principal.clone();
    stale.execution.connection_generation += 1;
    assert!(
        worker_authority(&missing.hub, &stale, &missing.request)
            .await
            .unwrap_err()
            .to_string()
            .contains("stale Agent connection")
    );
    let mut missing = missing;
    stop_worker_fixture(&mut missing).await;

    let mut parent_stale = worker_fixture(true, true).await;
    assert!(
        parent_stale
            .hub
            .lock()
            .await
            .registry
            .unregister_exact(&parent_stale.root)
    );
    assert!(
        worker_authority(
            &parent_stale.hub,
            &parent_stale.principal,
            &parent_stale.request,
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("parent authority is stale")
    );
    stop_worker_fixture(&mut parent_stale).await;

    let mut invalid_parent = worker_fixture(true, true).await;
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.session_id = None;
    assert!(
        invalid_parent
            .hub
            .lock()
            .await
            .registry
            .set_runtime_facts(&invalid_parent.root, facts)
    );
    assert!(
        worker_authority(
            &invalid_parent.hub,
            &invalid_parent.principal,
            &invalid_parent.request,
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("session is not bound")
    );
    stop_worker_fixture(&mut invalid_parent).await;

    let unavailable = worker_fixture(false, true).await;
    assert!(
        worker_authority(
            &unavailable.hub,
            &unavailable.principal,
            &unavailable.request
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("backend is unavailable")
    );
}
