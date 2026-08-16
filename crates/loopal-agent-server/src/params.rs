use std::sync::Arc;

use loopal_agent::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_config::ResolvedConfig;
use loopal_decision_api::DecisionMode;
use loopal_kernel::Kernel;
use loopal_runtime::AgentLoopParams;
use loopal_scheduler::CronScheduler;

use crate::mcp_settle::{mcp_startup_wait, spawn_proxy_mcp_settle_poll};
use provider_secrets::expand_provider_secrets;

mod provider_secrets;

pub struct AgentSetupResult {
    pub params: AgentLoopParams,
    pub task_store: Arc<TaskStore>,
    pub scheduler: Arc<CronScheduler>,
    pub agent_shared: Arc<AgentShared>,
}

#[derive(Default)]
pub struct StartParams {
    #[allow(dead_code)]
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub prompt: Option<String>,
    pub permission_mode: Option<String>,
    pub decision_mode: Option<String>,
    pub no_sandbox: bool,
    pub sandbox_policy: Option<loopal_config::SandboxPolicy>,
    pub session_id: Option<uuid::Uuid>,
    pub workflow_permission_causation: Option<loopal_protocol::WorkflowPermissionCausation>,
    pub workflow_attempt_capability: Option<loopal_protocol::WorkflowAttemptCapability>,
    pub workflow_completion_result_limit: Option<u32>,
    pub resume: Option<String>,
    pub lifecycle: loopal_runtime::LifecycleMode,
    pub agent_type: Option<String>,
    pub depth: Option<u32>,
    pub fork_context: Option<serde_json::Value>,
}

pub(crate) struct WorkflowProviderSecretAuthority {
    pub(crate) causation: loopal_protocol::WorkflowPermissionCausation,
    pub(crate) capability: loopal_protocol::WorkflowAttemptCapability,
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
    build_kernel_from_config_with_workflow_provider(
        config,
        production,
        depth,
        hub_client,
        hub_connection,
        cwd,
        agent_name,
        session_id,
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn build_kernel_from_config_with_workflow_provider(
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
    workflow_provider: Option<WorkflowProviderSecretAuthority>,
) -> anyhow::Result<Arc<Kernel>> {
    let mut settings = config.settings.clone();
    let cwd_for_memory = cwd.clone();
    let redaction_seed = loopal_output_guard::FinalSinkRedactionSeed::new();
    let secret_client: Option<Arc<dyn loopal_secret_client::SecretClient>> =
        hub_connection.as_ref().map(|conn| {
            Arc::new(
                loopal_secret_client::HubSecretClient::new(
                    conn.clone(),
                    cwd.clone(),
                    agent_name,
                    depth,
                )
                .with_final_sink_redaction_seed(redaction_seed.clone()),
            ) as Arc<dyn loopal_secret_client::SecretClient>
        });
    if let Some(authority) = workflow_provider {
        let connection = hub_connection.as_ref().ok_or_else(|| {
            anyhow::anyhow!("workflow provider secret authority requires Hub IPC")
        })?;
        let client = loopal_secret_client::HubSecretClient::new_workflow_provider(
            connection.clone(),
            cwd,
            authority.causation,
            authority.capability,
        )
        .with_final_sink_redaction_seed(redaction_seed);
        expand_provider_secrets(&mut settings, &client).await;
    } else if let Some(client) = secret_client.as_ref() {
        expand_provider_secrets(&mut settings, client.as_ref()).await;
    }
    let settings_memory = settings.memory.clone();
    let workflow_tools_enabled = depth == 0
        && settings.workflow.execution_enabled
        && settings.workflow.policy != loopal_config::OrchestrationPolicy::Off;
    let mut kernel = Kernel::new(settings)?;
    if let Some(client) = secret_client {
        kernel.set_secret_client(client);
    }
    if depth == 0 {
        kernel.register_goal_tools();
        if workflow_tools_enabled {
            loopal_agent::tools::register_workflow(&kernel);
        }
    }
    if production {
        let uses_proxy = hub_client.is_some();
        if let Some(client) = hub_client {
            let proxy = loopal_mcp::McpProxyClient::new(client);
            kernel.set_mcp_provider(Arc::new(proxy));
        }
        if depth == 0 || !uses_proxy {
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
    log_unresolvable_routing(&kernel);

    if production {
        spawn_proxy_mcp_settle_poll(Arc::downgrade(&kernel));
    }

    Ok(kernel)
}

/// Logging policy for unresolvable routes: the unreachable main model is an
/// error (LLM calls will fail), a bad `model_routing` entry is a warning (it
/// falls back to the main model). Soft by design — never aborts startup, since
/// sub-agent/proxy and test paths legitimately register fewer providers.
fn log_unresolvable_routing(kernel: &Kernel) {
    for entry in kernel.unresolvable_models() {
        match entry.slot {
            loopal_kernel::RoutedSlot::MainModel => tracing::error!(
                model = %entry.model,
                "configured main model resolves to no registered provider; \
                 LLM calls will fail — configure its provider or set `model`"
            ),
            loopal_kernel::RoutedSlot::Task(task) => tracing::warn!(
                model = %entry.model,
                ?task,
                "model_routing entry resolves to no registered provider; \
                 that task will fail when invoked"
            ),
        }
    }
}

pub fn build_kernel_with_provider(
    provider: Arc<dyn loopal_provider_api::Provider>,
    model_override: Option<&str>,
) -> anyhow::Result<Arc<Kernel>> {
    let mut settings = loopal_config::Settings::default();
    if let Some(model) = model_override {
        settings.model = model.to_string();
    }
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
    if let Some(policy) = start.sandbox_policy {
        settings.sandbox.policy = policy;
    } else if start.no_sandbox {
        settings.sandbox.policy = loopal_config::SandboxPolicy::Disabled;
    }
}
