use std::sync::Arc;
use std::time::Duration;

use indexmap::IndexMap;
use loopal_config::McpServerConfig;
use loopal_mcp::{LocalMcpProvider, McpManager, McpProvider};
use tokio::sync::RwLock;

fn make_provider() -> LocalMcpProvider {
    LocalMcpProvider::new(Arc::new(RwLock::new(McpManager::new())))
}

#[tokio::test]
async fn wait_until_settled_returns_true_immediately_when_no_spawn() {
    let provider = make_provider();
    let start = std::time::Instant::now();
    let settled = provider.wait_until_settled(Duration::from_secs(1)).await;
    assert!(settled);
    assert!(start.elapsed() < Duration::from_millis(50));
}

#[tokio::test]
async fn spawn_background_does_not_block_on_slow_server() {
    let provider = make_provider();
    let mut configs = IndexMap::new();
    configs.insert(
        "slow".to_string(),
        McpServerConfig::Stdio {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), "sleep 10".to_string()],
            env: Default::default(),
            enabled: true,
            timeout_ms: 1000,
        },
    );

    let start = std::time::Instant::now();
    provider.spawn_background(configs);
    let spawn_elapsed = start.elapsed();
    assert!(
        spawn_elapsed < Duration::from_millis(100),
        "spawn_background must return immediately, took {spawn_elapsed:?}"
    );
}

#[tokio::test]
async fn wait_until_settled_times_out_for_slow_server() {
    let provider = make_provider();
    let mut configs = IndexMap::new();
    configs.insert(
        "very-slow".to_string(),
        McpServerConfig::Stdio {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), "sleep 30".to_string()],
            env: Default::default(),
            enabled: true,
            timeout_ms: 60_000,
        },
    );
    provider.spawn_background(configs);

    let start = std::time::Instant::now();
    let settled = provider
        .wait_until_settled(Duration::from_millis(300))
        .await;
    let elapsed = start.elapsed();
    assert!(!settled, "should time out, not settle");
    assert!(
        elapsed >= Duration::from_millis(300),
        "wait should honor timeout, elapsed {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(800),
        "wait should not exceed timeout much, elapsed {elapsed:?}"
    );
}

#[tokio::test]
async fn wait_until_settled_returns_true_after_failed_server_settles() {
    let provider = make_provider();
    let mut configs = IndexMap::new();
    configs.insert(
        "bad".to_string(),
        McpServerConfig::Stdio {
            command: "__definitely_not_a_real_binary__".to_string(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            timeout_ms: 500,
        },
    );
    provider.spawn_background(configs);
    let settled = provider.wait_until_settled(Duration::from_secs(3)).await;
    assert!(settled, "failed server should still mark settled");
    let snaps = provider.snapshot().await;
    assert_eq!(snaps.len(), 1);
    assert!(snaps[0].status.starts_with("failed"));
}

#[tokio::test]
async fn spawn_background_with_empty_configs_is_noop() {
    let provider = make_provider();
    let configs = IndexMap::<String, McpServerConfig>::new();
    provider.spawn_background(configs);
    assert!(provider.wait_until_settled(Duration::from_millis(50)).await);
}

#[tokio::test]
async fn wait_until_settled_honors_all_overlapping_spawns() {
    let provider = make_provider();
    let mut slow = IndexMap::new();
    slow.insert(
        "slow-a".to_string(),
        McpServerConfig::Stdio {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), "sleep 3".to_string()],
            env: Default::default(),
            enabled: true,
            timeout_ms: 200,
        },
    );
    let mut fast = IndexMap::new();
    fast.insert(
        "fail-fast".to_string(),
        McpServerConfig::Stdio {
            command: "__definitely_missing_binary__".to_string(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            timeout_ms: 100,
        },
    );

    provider.spawn_background(slow);
    tokio::time::sleep(Duration::from_millis(20)).await;
    provider.spawn_background(fast);

    let settled_short = provider
        .wait_until_settled(Duration::from_millis(100))
        .await;
    assert!(
        !settled_short,
        "must NOT report settled while the slow spawn is still in-flight, \
         even though the fast spawn finished — generation counter guarantee"
    );

    let settled_full = provider.wait_until_settled(Duration::from_secs(5)).await;
    assert!(settled_full, "both spawns should eventually settle");
}

#[tokio::test]
async fn call_tool_retries_after_transport_closed() {
    let provider = make_provider();
    let mut configs = IndexMap::new();
    configs.insert(
        "ghost".to_string(),
        McpServerConfig::Stdio {
            command: "__no_such_binary__".to_string(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            timeout_ms: 300,
        },
    );
    provider.spawn_background(configs);
    assert!(provider.wait_until_settled(Duration::from_secs(3)).await);

    let snaps = provider.snapshot().await;
    assert_eq!(snaps.len(), 1, "failed server still kept in connections");
    assert!(snaps[0].status.starts_with("failed"));

    let result = provider
        .call_tool("ghost", "anything", &serde_json::json!({}))
        .await;
    assert!(
        matches!(
            result,
            Err(loopal_error::McpError::TransportClosed(_))
                | Err(loopal_error::McpError::ConnectionFailed(_))
        ),
        "transport closed → try_reconnect → retry → still fails because the binary doesn't exist, got {result:?}"
    );
}

#[tokio::test]
async fn call_tool_on_unknown_server_returns_server_not_found() {
    let provider = make_provider();
    let result = provider
        .call_tool("never-registered", "t", &serde_json::json!({}))
        .await;
    assert!(matches!(
        result,
        Err(loopal_error::McpError::ServerNotFound(_))
    ));
}

#[tokio::test]
async fn await_all_settled_unblocks_after_background_spawn_finishes() {
    let provider = make_provider();
    let mut configs = IndexMap::new();
    configs.insert(
        "fail-fast".to_string(),
        McpServerConfig::Stdio {
            command: "__definitely_missing__".to_string(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            timeout_ms: 200,
        },
    );
    provider.spawn_background(configs);

    let start = std::time::Instant::now();
    provider.await_all_settled().await;
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(3),
        "await_all_settled must release after the spawn finishes (even via failure), took {elapsed:?}"
    );
}

#[tokio::test]
async fn await_all_settled_returns_immediately_when_no_pending_spawn() {
    let provider = make_provider();
    let start = std::time::Instant::now();
    provider.await_all_settled().await;
    assert!(start.elapsed() < Duration::from_millis(20));
}

#[tokio::test]
async fn await_all_settled_waits_through_multiple_overlapping_spawns() {
    let provider = make_provider();
    let mut spawn_a = IndexMap::new();
    spawn_a.insert(
        "a".to_string(),
        McpServerConfig::Stdio {
            command: "__missing_a__".to_string(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            timeout_ms: 100,
        },
    );
    let mut spawn_b = IndexMap::new();
    spawn_b.insert(
        "b".to_string(),
        McpServerConfig::Stdio {
            command: "__missing_b__".to_string(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            timeout_ms: 200,
        },
    );
    provider.spawn_background(spawn_a);
    provider.spawn_background(spawn_b);

    let start = std::time::Instant::now();
    provider.await_all_settled().await;
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "must release only after BOTH spawns finish, took {elapsed:?}"
    );
    let snaps = provider.snapshot().await;
    assert_eq!(snaps.len(), 2, "both servers should be persisted as failed");
}
