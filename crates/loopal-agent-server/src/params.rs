use std::sync::Arc;

use loopal_agent::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_config::ResolvedConfig;
use loopal_decision_api::DecisionMode;
use loopal_kernel::Kernel;
use loopal_runtime::AgentLoopParams;
use loopal_scheduler::CronScheduler;

use crate::mcp_settle::{mcp_startup_wait, spawn_proxy_mcp_settle_poll};

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

#[allow(clippy::too_many_arguments)]
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
    session_id: String,
) -> anyhow::Result<Arc<Kernel>> {
    let mut settings = config.settings.clone();
    let cwd_for_memory = cwd.clone();
    let secret_client: Option<Arc<dyn loopal_secret_client::SecretClient>> =
        hub_connection.map(|conn| {
            Arc::new(loopal_secret_client::HubSecretClient::new(
                conn, cwd, agent_name, depth,
            )) as Arc<dyn loopal_secret_client::SecretClient>
        });
    if let Some(client) = secret_client.as_ref() {
        expand_provider_secrets(&mut settings, client.as_ref()).await;
    }
    let settings_memory = settings.memory.clone();
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
    // reason: 所有 depth 都注册 memory_recall — sub-agent 的 system prompt 也注入了
    //  memory-guidance.md "always use memory_recall" 指令，若仅 root 注册会让 sub-agent
    //  看到指令但找不到工具。memory.db 落在 ~/.loopal/sessions/{id}/memory.db，是
    //  per-session 派生数据；不与 .loopal/memory/*.md（user SSOT）混居。
    crate::memory_init::init_project_memory(
        &mut kernel,
        &cwd_for_memory,
        &session_id,
        &settings_memory,
    )
    .await;
    let kernel = Arc::new(kernel);

    if production {
        spawn_proxy_mcp_settle_poll(Arc::downgrade(&kernel));
    }

    Ok(kernel)
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
    // Critical-path (agent/start) but reverse-IPC dispatcher is already
    // Listening per P2-A's happens-before; 8s budget bounds any stuck hub
    // call so we surface "timed out" rather than the 30s handshake timeout.
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
