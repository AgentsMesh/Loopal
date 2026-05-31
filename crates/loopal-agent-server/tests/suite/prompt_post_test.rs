use std::time::Instant;

use indexmap::IndexMap;
use loopal_agent_server::params::build_kernel_from_config;
use loopal_config::{ConfigResolver, McpServerConfig, ResolvedConfig};

fn empty_config() -> ResolvedConfig {
    ConfigResolver::new().resolve().unwrap()
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

#[tokio::test]
async fn append_runtime_sections_lists_configured_servers_even_when_not_yet_ready() {
    // User-impact regression: when chrome-devtools-mcp (~30s startup) misses
    // the bounded-wait budget, its tools aren't in tool_definitions at
    // system-prompt time. Without explicit "MCP Server Status" section the
    // LLM would tell the user "I don't have that tool" — even though the
    // user configured it and it WILL become available shortly. This test
    // pins down the awareness: the configured server name MUST appear in
    // the prompt regardless of whether finalize observed it ready.
    let mut servers = IndexMap::new();
    servers.insert(
        "expected-by-user".to_string(),
        stdio_server("sh", vec!["-c", "sleep 30"], 60_000),
    );
    let config = config_with_mcp_servers(servers);

    unsafe { std::env::set_var("LOOPAL_MCP_STARTUP_WAIT_SECS", "0") };
    let start = Instant::now();
    let kernel = build_kernel_from_config(
        &config,
        true,
        0,
        None,
        None,
        std::path::PathBuf::from("."),
        "test".to_string(),
        "test-session".to_string(),
    )
    .await
    .expect("build");
    let elapsed = start.elapsed();
    unsafe { std::env::remove_var("LOOPAL_MCP_STARTUP_WAIT_SECS") };

    // Sanity: build returned within bounded wait budget (not blocked).
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "build must not block on slow server, took {elapsed:?}"
    );

    let mut prompt = String::new();
    loopal_agent_server::prompt_post::append_runtime_sections(&mut prompt, &kernel).await;
    assert!(
        prompt.contains("expected-by-user"),
        "system prompt must list configured server name even when not ready, got: {prompt}"
    );
    assert!(
        prompt.contains("MCP Server Status"),
        "status section header must be present"
    );
    assert!(
        prompt.contains("`/mcp`") || prompt.contains("/mcp"),
        "prompt should suggest checking /mcp page"
    );
}

#[tokio::test]
async fn append_runtime_sections_omits_status_when_no_servers_configured() {
    let config = empty_config();
    let kernel = build_kernel_from_config(
        &config,
        true,
        0,
        None,
        None,
        std::path::PathBuf::from("."),
        "test".to_string(),
        "test-session".to_string(),
    )
    .await
    .expect("build");
    let mut prompt = String::new();
    loopal_agent_server::prompt_post::append_runtime_sections(&mut prompt, &kernel).await;
    assert!(
        !prompt.contains("MCP Server Status"),
        "status section should not appear when user configured no MCP servers"
    );
}

#[tokio::test]
#[ignore = "obsolete after Stage 2: MCP now lives in Hub, not root agent"]
async fn append_runtime_sections_shows_failed_status_for_dead_binary() {
    let mut servers = IndexMap::new();
    servers.insert(
        "definitely-fails".to_string(),
        stdio_server("__no_such_binary__", Vec::new(), 200),
    );
    let config = config_with_mcp_servers(servers);

    let kernel = build_kernel_from_config(
        &config,
        true,
        0,
        None,
        None,
        std::path::PathBuf::from("."),
        "test".to_string(),
        "test-session".to_string(),
    )
    .await
    .expect("build");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let mut prompt = String::new();
    loopal_agent_server::prompt_post::append_runtime_sections(&mut prompt, &kernel).await;
    assert!(
        prompt.contains("definitely-fails"),
        "failed server still listed so LLM doesn't pretend the user didn't configure it"
    );
    assert!(
        prompt.contains("failed"),
        "status should clearly indicate failure: {prompt}"
    );
}
