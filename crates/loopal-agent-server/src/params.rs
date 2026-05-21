use std::sync::Arc;

use loopal_agent::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_config::ResolvedConfig;
use loopal_decision_api::DecisionMode;
use loopal_kernel::Kernel;
use loopal_runtime::AgentLoopParams;
use loopal_scheduler::CronScheduler;

pub struct AgentSetupResult {
    pub params: AgentLoopParams,
    pub task_store: Arc<TaskStore>,
    pub scheduler: Arc<CronScheduler>,
    pub agent_shared: Arc<AgentShared>,
}

pub struct StartParams {
    #[allow(dead_code)]
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub prompt: Option<String>,
    pub permission_mode: Option<String>,
    pub decision_mode: Option<String>,
    pub no_sandbox: bool,
    pub resume: Option<String>,
    pub lifecycle: loopal_runtime::LifecycleMode,
    pub agent_type: Option<String>,
    pub depth: Option<u32>,
    pub fork_context: Option<serde_json::Value>,
}

pub async fn build_kernel_from_config(
    config: &ResolvedConfig,
    production: bool,
    depth: u32,
    hub_client: Option<Arc<dyn loopal_mcp::HubMcpClient>>,
    hub_connection: Option<
        Arc<loopal_ipc::connection::Connection<loopal_ipc::connection::Listening>>,
    >,
    cwd: std::path::PathBuf,
    agent_name: String,
) -> anyhow::Result<Arc<Kernel>> {
    let mut settings = config.settings.clone();
    let secret_client: Option<Arc<dyn loopal_secret_client::SecretClient>> =
        hub_connection.map(|conn| {
            Arc::new(loopal_secret_client::HubSecretClient::new(
                conn, cwd, agent_name, depth,
            )) as Arc<dyn loopal_secret_client::SecretClient>
        });
    if let Some(client) = secret_client.as_ref() {
        expand_provider_secrets(&mut settings, client.as_ref()).await;
    }
    let mut kernel = Kernel::new(settings)?;
    if let Some(client) = secret_client {
        kernel.set_secret_client(client);
    }
    if depth == 0 {
        kernel.register_goal_tools();
    }
    if production {
        if let Some(client) = hub_client {
            let proxy = loopal_mcp::McpProxyClient::new(client);
            kernel.set_mcp_provider(Arc::new(proxy));
        }
        let wait = mcp_startup_wait();
        let settled = tokio::time::timeout(
            wait + std::time::Duration::from_secs(1),
            kernel.finalize_mcp_tools(wait),
        )
        .await
        .unwrap_or(false);
        if !settled {
            tracing::warn!(
                wait_secs = wait.as_secs(),
                "MCP startup did not settle in time; slow servers will register later"
            );
        }
    }
    loopal_agent::tools::register_all(&mut kernel);
    let kernel = Arc::new(kernel);

    if production {
        spawn_proxy_mcp_settle_poll(Arc::downgrade(&kernel));
    }

    Ok(kernel)
}

/// reason: agent uses `McpProxyClient` and cannot subscribe to Hub-side
/// `LocalMcpProvider` settle events directly. Poll the Hub at low cadence
/// while tools are still arriving; back off and terminate once the tool
/// surface is stable so we don't tick forever for the kernel's lifetime.
/// `register_all_settled_mcp_tools` returns the number of newly registered
/// adapters, so a streak of zeros means "settled".
fn spawn_proxy_mcp_settle_poll(kernel: std::sync::Weak<Kernel>) {
    let poll = mcp_settle_poll_interval();
    let quiet_streak_to_settle: u32 = std::env::var("LOOPAL_MCP_POLL_QUIET_STREAK")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(4);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(poll);
        interval.tick().await;
        let mut quiet_streak: u32 = 0;
        loop {
            interval.tick().await;
            let Some(k) = kernel.upgrade() else {
                return;
            };
            let added = k.register_all_settled_mcp_tools().await;
            if added == 0 {
                quiet_streak = quiet_streak.saturating_add(1);
                if quiet_streak >= quiet_streak_to_settle {
                    tracing::debug!(
                        quiet_streak,
                        "proxy MCP settle-poll: tool surface stable, terminating"
                    );
                    return;
                }
            } else {
                quiet_streak = 0;
            }
        }
    });
}

fn mcp_startup_wait() -> std::time::Duration {
    let secs = std::env::var("LOOPAL_MCP_STARTUP_WAIT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5);
    std::time::Duration::from_secs(secs)
}

fn mcp_settle_poll_interval() -> std::time::Duration {
    let secs = std::env::var("LOOPAL_MCP_POLL_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(3);
    std::time::Duration::from_secs(secs)
}

pub fn build_kernel_with_provider(
    provider: Arc<dyn loopal_provider_api::Provider>,
) -> anyhow::Result<Arc<Kernel>> {
    let settings = loopal_config::Settings::default();
    let mut kernel = Kernel::new(settings)?;
    loopal_agent::tools::register_all(&mut kernel);
    kernel.register_provider(provider);
    Ok(Arc::new(kernel))
}

pub fn apply_start_overrides(settings: &mut loopal_config::Settings, start: &StartParams) {
    if let Some(ref model) = start.model {
        settings.model = model.clone();
    }
    if let Some(ref mode) = start.permission_mode {
        if let Ok(parsed) = mode.parse::<loopal_tool_api::PermissionMode>() {
            settings.permission_mode = parsed;
        } else {
            tracing::warn!(input = %mode, "invalid permission_mode, ignoring");
        }
    }
    if let Some(ref decision) = start.decision_mode {
        if let Ok(parsed) = decision.parse::<DecisionMode>() {
            settings.decision_mode = parsed;
        } else {
            tracing::warn!(input = %decision, "invalid decision_mode, ignoring");
        }
    }
    if start.no_sandbox {
        settings.sandbox.policy = loopal_config::SandboxPolicy::Disabled;
    }
}

async fn expand_provider_secrets(
    settings: &mut loopal_config::Settings,
    client: &dyn loopal_secret_client::SecretClient,
) {
    // reason: critical path (agent/start) but reverse-IPC dispatcher is
    // already Listening by P2-A's happens-before; default 8s budget bounds
    // any stuck hub call so we surface "timed out" rather than the layered
    // 30s handshake timeout further up.
    let budget = loopal_ipc::HUB_RPC_BUDGET;
    for slot in [
        &mut settings.providers.anthropic,
        &mut settings.providers.openai,
        &mut settings.providers.google,
    ] {
        if let Some(cfg) = slot.as_mut() {
            if let Some(k) = cfg.api_key.as_mut() {
                *k = loopal_secret_runtime::expand_to_plaintext(k, client, budget).await;
            }
            if let Some(u) = cfg.base_url.as_mut() {
                *u = loopal_secret_runtime::expand_to_plaintext(u, client, budget).await;
            }
        }
    }
    for cfg in settings.providers.openai_compat.iter_mut() {
        cfg.base_url =
            loopal_secret_runtime::expand_to_plaintext(&cfg.base_url, client, budget).await;
        if let Some(k) = cfg.api_key.as_mut() {
            *k = loopal_secret_runtime::expand_to_plaintext(k, client, budget).await;
        }
    }
}
