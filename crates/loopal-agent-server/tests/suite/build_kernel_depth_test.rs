use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use indexmap::IndexMap;
use loopal_agent_server::params::build_kernel_from_config;
use loopal_config::{ConfigResolver, McpServerConfig, ResolvedConfig};
use loopal_mcp::HubMcpClient;
use serde_json::Value;

fn empty_config() -> ResolvedConfig {
    ConfigResolver::new()
        .resolve()
        .expect("empty resolver should produce default ResolvedConfig")
}

fn config_with_mcp_servers(servers: IndexMap<String, McpServerConfig>) -> ResolvedConfig {
    let mut cfg = empty_config();
    cfg.settings.mcp_servers = servers;
    cfg
}

fn stdio_server(command: &str, args: Vec<&str>, timeout_ms: u64) -> McpServerConfig {
    McpServerConfig::Stdio {
        command: command.to_string(),
        args: args.into_iter().map(String::from).collect(),
        env: Default::default(),
        enabled: true,
        timeout_ms,
        sharing: Default::default(),
        cwd_isolation: None,
    }
}

struct NoopHubClient;

#[async_trait]
impl HubMcpClient for NoopHubClient {
    async fn send_request(&self, _method: &str, _params: Value) -> Result<Value, String> {
        Err("no remote".into())
    }
}

async fn build_or_panic(
    config: &ResolvedConfig,
    production: bool,
    depth: u32,
    hub_client: Option<Arc<dyn HubMcpClient>>,
) -> Arc<loopal_kernel::Kernel> {
    build_kernel_from_config(
        config,
        production,
        depth,
        hub_client,
        None,
        std::path::PathBuf::from("."),
        "test".to_string(),
    )
    .await
    .expect("build_kernel_from_config")
}

// SAFETY: env mutation is process-global; tokio::test default is single-threaded.
fn set_short_startup_wait() {
    unsafe { std::env::set_var("LOOPAL_MCP_STARTUP_WAIT_SECS", "1") };
}

fn unset_short_startup_wait() {
    unsafe { std::env::remove_var("LOOPAL_MCP_STARTUP_WAIT_SECS") };
}

#[tokio::test]
async fn depth_zero_uses_local_backend_with_manager() {
    let kernel = build_or_panic(&empty_config(), true, 0, None).await;
    assert!(
        kernel.mcp_manager().is_some(),
        "depth=0 must select LocalMcpProvider (mcp_manager()==Some)"
    );
}

#[tokio::test]
async fn depth_gt0_with_hub_client_uses_proxy_backend() {
    let hub_client: Arc<dyn HubMcpClient> = Arc::new(NoopHubClient);
    let kernel = build_or_panic(&empty_config(), true, 1, Some(hub_client)).await;
    assert!(
        kernel.mcp_manager().is_none(),
        "depth>0 with hub_client must inject McpProxyClient (no local manager)"
    );
}

#[tokio::test]
async fn depth_gt0_without_hub_client_falls_back_to_local() {
    let kernel = build_or_panic(&empty_config(), true, 1, None).await;
    assert!(
        kernel.mcp_manager().is_some(),
        "depth>0 without hub_client must fall back to LocalMcpProvider"
    );
}

#[tokio::test]
async fn non_production_skips_mcp_entirely() {
    let kernel = build_or_panic(&empty_config(), false, 0, None).await;
    assert!(
        kernel.mcp_manager().is_some(),
        "kernel always starts with Local backend; non-production just skips spawn_mcp"
    );
}

// Core startup-resilience guarantee: bounded-wait budget bounds agent/start
// even when configured MCP servers would otherwise stall connect().
#[tokio::test]
async fn build_kernel_with_slow_mcp_server_returns_within_bounded_wait() {
    let mut servers = IndexMap::new();
    servers.insert(
        "stuck-server".to_string(),
        stdio_server("sh", vec!["-c", "sleep 30"], 60_000),
    );
    let config = config_with_mcp_servers(servers);

    set_short_startup_wait();
    let start = Instant::now();
    let kernel = build_or_panic(&config, true, 0, None).await;
    let elapsed = start.elapsed();
    unset_short_startup_wait();

    assert!(
        elapsed < Duration::from_secs(3),
        "bounded_wait=1s + overhead must be ≤3s, took {elapsed:?}"
    );
    assert!(
        kernel.mcp_manager().is_some(),
        "root kernel still owns its local provider after timeout"
    );
}

#[tokio::test]
async fn build_kernel_mixed_servers_completes_within_bounded_wait() {
    let mut servers = IndexMap::new();
    servers.insert(
        "slow".to_string(),
        stdio_server("sh", vec!["-c", "sleep 30"], 60_000),
    );
    servers.insert(
        "bad".to_string(),
        stdio_server("__no_such_binary__", Vec::new(), 300),
    );
    let config = config_with_mcp_servers(servers);

    set_short_startup_wait();
    let start = Instant::now();
    let _ = build_or_panic(&config, true, 0, None).await;
    let elapsed = start.elapsed();
    unset_short_startup_wait();

    assert!(
        elapsed < Duration::from_secs(3),
        "even with multiple problem servers, build must respect bounded wait, took {elapsed:?}"
    );
}

// Anti-process-explosion: a sub-agent (depth>0 + hub_client) must NOT spawn
// MCP servers locally even when the config lists slow ones — that's the
// chrome-devtools-mcp duplication scenario.
#[tokio::test]
async fn sub_agent_build_with_slow_root_config_does_not_spawn_local_mcp() {
    let mut servers = IndexMap::new();
    servers.insert(
        "would-be-stuck".to_string(),
        stdio_server("sh", vec!["-c", "sleep 30"], 60_000),
    );
    let config = config_with_mcp_servers(servers);

    let hub_client: Arc<dyn HubMcpClient> = Arc::new(NoopHubClient);
    let start = Instant::now();
    let kernel = build_or_panic(&config, true, 1, Some(hub_client)).await;
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "sub-agent in proxy mode must skip local MCP spawn entirely; took {elapsed:?}"
    );
    assert!(
        kernel.mcp_manager().is_none(),
        "sub-agent must not own a local manager"
    );
}
