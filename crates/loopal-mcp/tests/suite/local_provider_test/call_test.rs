#[tokio::test]
async fn provider_exposes_manager_and_settle_subscription() {
    let provider = make_provider();
    assert!(
        provider
            .manager()
            .read()
            .await
            .collect_snapshots()
            .is_empty()
    );
    assert_eq!(*provider.subscribe_settle_events().borrow(), 0);
    assert!(!provider.has_server("missing").await);
    assert!(provider.list_tools(HUB_RPC_BUDGET).await.is_empty());
}

#[tokio::test]
async fn call_tool_retries_after_transport_closed() {
    let provider = make_provider();
    provider.spawn_background(IndexMap::from([(
        "ghost".into(),
        failing_config("__no_such_binary__", 300),
    )]));
    assert!(provider.wait_until_settled(Duration::from_secs(3)).await);
    let result = provider
        .call_tool("ghost", "anything", &serde_json::json!({}), HUB_RPC_BUDGET)
        .await;
    assert!(matches!(
        result,
        Err(loopal_error::McpError::TransportClosed(_))
            | Err(loopal_error::McpError::ConnectionFailed(_))
    ));
}

#[tokio::test]
async fn unknown_server_paths_fail_without_reconnect() {
    let provider = make_provider();
    assert!(!provider.try_reconnect("never-registered").await);
    let result = provider
        .call_tool(
            "never-registered",
            "tool",
            &serde_json::json!({}),
            HUB_RPC_BUDGET,
        )
        .await;
    assert!(matches!(
        result,
        Err(loopal_error::McpError::ServerNotFound(_))
    ));
}
