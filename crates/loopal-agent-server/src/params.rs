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
    /// JSON-encoded `{"mode": "<permission_mode>", "decision": "<decision_mode>"}`
    /// (e.g. `{"mode":"ask_dangerous","decision":"auto"}`).
    /// Both fields are required; see `parse_permission_argv`.
    pub permission: Option<String>,
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

pub(crate) fn apply_start_overrides(settings: &mut loopal_config::Settings, start: &StartParams) {
    if let Some(ref model) = start.model {
        settings.model = model.clone();
    }
    if let Some(ref perm) = start.permission {
        match parse_permission_argv(perm) {
            Ok((mode, decision)) => {
                settings.permission_mode = mode;
                settings.decision_mode = decision;
            }
            Err(e) => {
                tracing::warn!(input = %perm, error = %e, "invalid permission spawn arg, ignoring");
            }
        }
    }
    if start.no_sandbox {
        settings.sandbox.policy = loopal_config::SandboxPolicy::Disabled;
    }
}

#[derive(serde::Deserialize)]
struct PermissionEncoding {
    mode: loopal_tool_api::PermissionMode,
    decision: DecisionMode,
}

pub fn parse_permission_argv(
    s: &str,
) -> Result<(loopal_tool_api::PermissionMode, DecisionMode), String> {
    let parsed: PermissionEncoding =
        serde_json::from_str(s).map_err(|e| format!("invalid permission JSON: {e}"))?;
    Ok((parsed.mode, parsed.decision))
}
