//! Agent loop parameter construction for the IPC server.

use std::sync::Arc;

use loopal_agent::task_store::TaskStore;
use loopal_config::ResolvedConfig;
use loopal_kernel::Kernel;
use loopal_runtime::AgentLoopParams;
use loopal_scheduler::CronScheduler;

/// Return value from `build_with_frontend` — agent loop params + bridge handles.
pub struct AgentSetupResult {
    pub params: AgentLoopParams,
    pub task_store: Arc<TaskStore>,
    pub scheduler: Arc<CronScheduler>,
}

pub struct StartParams {
    #[allow(dead_code)]
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub prompt: Option<String>,
    pub permission_mode: Option<String>,
    pub no_sandbox: bool,
    pub resume: Option<String>,
    /// Explicit lifecycle mode. Ephemeral exits on idle, Persistent waits.
    pub lifecycle: loopal_runtime::LifecycleMode,
    /// Agent type for fragment selection (e.g. "explore", "plan").
    pub agent_type: Option<String>,
    /// Nesting depth (0 = root). Propagated from parent via IPC.
    pub depth: Option<u32>,
    /// Fork context: compressed parent messages (JSON Value, deserialized in agent_setup).
    pub fork_context: Option<serde_json::Value>,
}

/// Build a Kernel from config (production path: MCP, tools).
/// Caller should apply start overrides to config.settings before calling.
pub(crate) async fn build_kernel_from_config(
    config: &ResolvedConfig,
    production: bool,
) -> anyhow::Result<Arc<Kernel>> {
    let mut kernel = Kernel::new(config.settings.clone())?;
    if production {
        // Wire up MCP sampling: resolve the default model's provider and inject.
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

/// Build a Kernel with injected provider (test path).
pub fn build_kernel_with_provider(
    provider: Arc<dyn loopal_provider_api::Provider>,
) -> anyhow::Result<Arc<Kernel>> {
    let settings = loopal_config::Settings::default();
    let mut kernel = Kernel::new(settings)?;
    loopal_agent::tools::register_all(&mut kernel);
    kernel.register_provider(provider);
    Ok(Arc::new(kernel))
}

/// Apply CLI overrides from StartParams to Settings before Kernel creation.
pub(crate) fn apply_start_overrides(settings: &mut loopal_config::Settings, start: &StartParams) {
    if let Some(ref model) = start.model {
        settings.model = model.clone();
    }
    if let Some(ref perm) = start.permission_mode {
        settings.permission_mode = match perm.as_str() {
            "bypass" | "yolo" => loopal_tool_api::PermissionMode::Bypass,
            "auto" => loopal_tool_api::PermissionMode::Auto,
            _ => loopal_tool_api::PermissionMode::Supervised,
        };
    }
    if start.no_sandbox {
        settings.sandbox.policy = loopal_config::SandboxPolicy::Disabled;
    }
}
