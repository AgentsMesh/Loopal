#[tokio::test]
async fn wait_until_settled_returns_true_immediately_when_no_spawn() {
    let provider = make_provider();
    let start = std::time::Instant::now();
    assert!(provider.wait_until_settled(Duration::from_secs(1)).await);
    assert!(start.elapsed() < Duration::from_millis(50));
}

#[tokio::test]
async fn spawn_background_does_not_block_on_slow_server() {
    let provider = make_provider();
    let config = McpServerConfig::Stdio {
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 10".into()],
        env: Default::default(),
        enabled: true,
        timeout_ms: 1000,
        sharing: Default::default(),
        cwd_isolation: None,
    };
    let start = std::time::Instant::now();
    provider.spawn_background(IndexMap::from([("slow".into(), config)]));
    assert!(start.elapsed() < Duration::from_millis(100));
}

#[tokio::test]
async fn wait_until_settled_times_out_for_slow_server() {
    let provider = make_provider();
    let config = McpServerConfig::Stdio {
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 30".into()],
        env: Default::default(),
        enabled: true,
        timeout_ms: 60_000,
        sharing: Default::default(),
        cwd_isolation: None,
    };
    provider.spawn_background(IndexMap::from([("very-slow".into(), config)]));
    let start = std::time::Instant::now();
    assert!(
        !provider
            .wait_until_settled(Duration::from_millis(300))
            .await
    );
    assert!(start.elapsed() >= Duration::from_millis(300));
    assert!(start.elapsed() < Duration::from_millis(800));
}

#[tokio::test]
async fn failed_server_settles_and_is_snapshotted() {
    let provider = make_provider();
    provider.spawn_background(IndexMap::from([(
        "bad".into(),
        failing_config("__definitely_not_a_real_binary__", 500),
    )]));
    assert!(provider.wait_until_settled(Duration::from_secs(3)).await);
    let snapshots = provider.snapshot(HUB_RPC_BUDGET).await;
    assert_eq!(snapshots.len(), 1);
    assert!(snapshots[0].status.starts_with("failed"));
}

#[tokio::test]
async fn empty_spawn_is_noop() {
    let provider = make_provider();
    provider.spawn_background(IndexMap::new());
    assert!(provider.wait_until_settled(Duration::from_millis(50)).await);
}

#[tokio::test]
async fn overlapping_spawns_settle_as_one_generation() {
    let provider = make_provider();
    let slow = McpServerConfig::Stdio {
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 3".into()],
        env: Default::default(),
        enabled: true,
        timeout_ms: 200,
        sharing: Default::default(),
        cwd_isolation: None,
    };
    provider.spawn_background(IndexMap::from([("slow-a".into(), slow)]));
    tokio::time::sleep(Duration::from_millis(20)).await;
    provider.spawn_background(IndexMap::from([(
        "fail-fast".into(),
        failing_config("__definitely_missing_binary__", 100),
    )]));
    assert!(
        !provider
            .wait_until_settled(Duration::from_millis(100))
            .await
    );
    assert!(provider.wait_until_settled(Duration::from_secs(5)).await);
}

#[tokio::test]
async fn await_all_settled_handles_none_one_and_multiple_spawns() {
    let provider = make_provider();
    provider.await_all_settled().await;
    provider.spawn_background(IndexMap::from([(
        "a".into(),
        failing_config("__missing_a__", 100),
    )]));
    provider.spawn_background(IndexMap::from([(
        "b".into(),
        failing_config("__missing_b__", 200),
    )]));
    provider.await_all_settled().await;
    assert_eq!(provider.snapshot(HUB_RPC_BUDGET).await.len(), 2);
}
