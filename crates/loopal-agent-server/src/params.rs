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

pub(crate) async fn build_kernel_from_config(
    config: &ResolvedConfig,
    production: bool,
    depth: u32,
) -> anyhow::Result<Arc<Kernel>> {
    let mut kernel = Kernel::new(config.settings.clone())?;
    if depth == 0 {
        kernel.register_goal_tools();
    }
    if production {
        if let Ok(provider) = kernel.resolve_provider(&config.settings.model) {
            let adapter =
                loopal_kernel::McpSamplingAdapter::new(provider, config.settings.model.clone());
            kernel
                .mcp_manager()
                .write()
                .await
                .set_sampling(Arc::new(adapter));
        }
        kernel.start_mcp().await?;
    }
    loopal_agent::tools::register_all(&mut kernel);
    Ok(Arc::new(kernel))
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
