//! Agent loop parameter construction for the IPC server.

use std::sync::Arc;

use loopal_config::ResolvedConfig;
use loopal_kernel::Kernel;

pub struct StartParams {
    #[allow(dead_code)]
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub prompt: Option<String>,
    pub permission_mode: Option<String>,
    pub no_sandbox: bool,
    pub resume: Option<String>,
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
            _ => loopal_tool_api::PermissionMode::Supervised,
        };
    }
    if start.no_sandbox {
        settings.sandbox.policy = loopal_config::SandboxPolicy::Disabled;
    }
}
