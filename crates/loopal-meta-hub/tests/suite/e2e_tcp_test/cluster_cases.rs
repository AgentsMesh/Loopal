/// Boot a real TCP cluster and route a message across hubs.
#[tokio::test]
async fn tcp_cluster_cross_hub_route() {
    let (addr, token, _meta_hub) = boot_meta_hub().await;
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();

    let _conn_a = join_hub_tcp(&hub_a, &addr, &token, "hub-a").await;
    let _conn_b = join_hub_tcp(&hub_b, &addr, &token, "hub-b").await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Register agent on Hub-B
    let (_agent_conn, _agent_rx) = register_mock(&hub_b, "target").await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Route from Hub-A to Hub-B's agent via uplink → MetaHub → Hub-B
    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "source": {"Agent": {"hub": [], "agent": "sender"}},
        "target": {"hub": ["hub-b"], "agent": "target"},
        "content": {"text": "hello via TCP", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_ROUTE.name,
        envelope,
        "sender".into(),
    )
    .await;
    assert!(
        result.is_ok(),
        "TCP cross-hub route should succeed: {result:?}"
    );
}

#[tokio::test]
async fn tcp_cluster_list_hubs() {
    let (addr, token, meta_hub) = boot_meta_hub().await;
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();
    let _conn_a = join_hub_tcp(&hub_a, &addr, &token, "hub-a").await;
    let _conn_b = join_hub_tcp(&hub_b, &addr, &token, "hub-b").await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let result = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta_hub,
        methods::META_LIST_HUBS.name,
        json!({}),
        "hub-a".into(),
    )
    .await
    .unwrap();
    let hubs = result["hubs"].as_array().unwrap();
    assert_eq!(hubs.len(), 2);
    let names: Vec<&str> = hubs.iter().filter_map(|h| h["name"].as_str()).collect();
    assert!(names.contains(&"hub-a"));
    assert!(names.contains(&"hub-b"));
}

#[tokio::test]
async fn dynamic_leave_and_immediate_rejoin_reuses_hub_name() {
    let (addr, token, meta_hub) = boot_meta_hub().await;
    let (hub, _) = make_hub();
    loopal_agent_hub::uplink_connection::connect(&hub, &addr, &token, "desktop-a")
        .await
        .unwrap();
    loopal_agent_hub::uplink_connection::disconnect(&hub)
        .await
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(1),
        loopal_agent_hub::uplink_connection::connect(&hub, &addr, &token, "desktop-a"),
    )
    .await
    .expect("rejoin must be bounded")
    .expect("same hub name should be reusable after explicit leave");
    assert_eq!(
        meta_hub.lock().await.registry.hub_names(),
        vec!["desktop-a"]
    );
}

#[tokio::test]
async fn dynamic_join_publishes_initial_agent_count_before_returning() {
    let (addr, token, meta_hub) = boot_meta_hub().await;
    let (hub, _) = make_hub();
    let (_agent, _messages) = register_mock(&hub, "main").await;
    loopal_agent_hub::uplink_connection::connect(&hub, &addr, &token, "desktop-a")
        .await
        .unwrap();
    let info = meta_hub.lock().await.registry.snapshot();
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].agent_count, 1);
}

#[tokio::test]
async fn dynamic_rejoin_recovers_a_stale_network_uplink() {
    let (addr, token, meta_hub) = boot_meta_hub().await;
    let (hub, _) = make_hub();
    loopal_agent_hub::uplink_connection::connect(&hub, &addr, &token, "desktop-a")
        .await
        .unwrap();
    let stale = hub.lock().await.uplink.clone().unwrap();
    stale.connection().close().await;
    tokio::time::timeout(
        Duration::from_secs(1),
        loopal_agent_hub::uplink_connection::connect(&hub, &addr, &token, "desktop-a"),
    )
    .await
    .expect("stale reconnect must be bounded")
    .expect("stale registration should be replaced");
    assert_eq!(
        meta_hub.lock().await.registry.hub_names(),
        vec!["desktop-a"]
    );
}
