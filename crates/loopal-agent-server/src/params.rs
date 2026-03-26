//! Agent loop parameter construction for the IPC server.

use std::sync::Arc;

use loopal_config::ResolvedConfig;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_kernel::Kernel;
use loopal_runtime::AgentLoopParams;

use loopal_provider_api::Provider;

use crate::agent_setup::build_inner;

pub(crate) struct StartParams {
    #[allow(dead_code)]
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub prompt: Option<String>,
    pub permission_mode: Option<String>,
    pub no_sandbox: bool,
}

/// Build agent params from config (production path).
pub(crate) async fn build(
    cwd: &std::path::Path,
    config: &ResolvedConfig,
    start: &StartParams,
    connection: &Arc<Connection>,
    incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
) -> anyhow::Result<AgentLoopParams> {
    let mut config = config.clone();
    apply_start_overrides(&mut config.settings, start);
    let mut kernel = Kernel::new(config.settings.clone())?;
    kernel.start_mcp().await?;
    loopal_agent::tools::register_all(&mut kernel);
    build_inner(
        cwd,
        &config,
        start,
        connection,
        incoming_rx,
        Arc::new(kernel),
        None,
        true,
    )
}

/// Build agent params with injected provider (test path).
pub(crate) fn build_with_provider(
    cwd: &std::path::Path,
    start: &StartParams,
    connection: &Arc<Connection>,
    incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
    provider: Arc<dyn Provider>,
    session_dir: &std::path::Path,
) -> anyhow::Result<AgentLoopParams> {
    let settings = loopal_config::Settings::default();
    let mut kernel = Kernel::new(settings.clone())?;
    loopal_agent::tools::register_all(&mut kernel);
    kernel.register_provider(provider);
    let config = ResolvedConfig {
        settings,
        mcp_servers: Default::default(),
        skills: Default::default(),
        hooks: Vec::new(),
        instructions: String::new(),
        memory: String::new(),
        layers: Vec::new(),
    };
    build_inner(
        cwd,
        &config,
        start,
        connection,
        incoming_rx,
        Arc::new(kernel),
        Some(session_dir),
        false,
    )
}

/// Apply CLI overrides from StartParams to Settings before Kernel creation.
fn apply_start_overrides(settings: &mut loopal_config::Settings, start: &StartParams) {
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
