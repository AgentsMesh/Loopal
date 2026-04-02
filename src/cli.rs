use clap::Parser;

#[derive(Parser)]
#[command(name = "loopal", about = "AI coding agent", version = "0.1.0")]
pub struct Cli {
    /// Model to use
    #[arg(short, long)]
    pub model: Option<String>,

    /// Resume a previous session
    #[arg(short, long)]
    pub resume: Option<String>,

    /// Permission mode
    #[arg(short = 'P', long)]
    pub permission: Option<String>,

    /// Start in plan mode
    #[arg(long)]
    pub plan: bool,

    /// Disable sandbox enforcement
    #[arg(long)]
    pub no_sandbox: bool,

    /// Run as ACP server for IDE integration (stdin/stdout JSON-RPC)
    #[arg(long)]
    pub acp: bool,

    /// Run without TUI (server mode, for CI/scripting/cluster workers)
    #[arg(long)]
    pub server: bool,

    /// Exit after completing current task (default: persistent)
    #[arg(long)]
    pub ephemeral: bool,

    /// Internal: run as agent worker process (stdin/stdout IPC)
    #[arg(long, hide = true)]
    pub serve: bool,

    /// Run agent in an isolated git worktree
    #[arg(long)]
    pub worktree: bool,

    /// [Testing] Path to JSON file with mock LLM responses.
    /// Can also be set via LOOPAL_TEST_PROVIDER env var.
    #[arg(long, hide = true)]
    pub test_provider: Option<String>,

    /// Run as MetaHub server (cluster coordinator)
    #[arg(long)]
    pub meta_hub: Option<String>,

    /// Join a MetaHub cluster (address:port)
    #[arg(long)]
    pub join_hub: Option<String>,

    /// Hub name when joining a MetaHub (defaults to hostname)
    #[arg(long)]
    pub hub_name: Option<String>,

    /// Initial prompt (non-interactive)
    pub prompt: Vec<String>,
}

impl Cli {
    /// Apply CLI flags to settings, overriding config-file values.
    pub fn apply_overrides(&self, settings: &mut loopal_config::Settings) {
        if let Some(model) = &self.model {
            settings.model = model.clone();
        }
        if let Some(perm) = &self.permission {
            settings.permission_mode = match perm.as_str() {
                "bypass" | "yolo" => loopal_tool_api::PermissionMode::Bypass,
                "auto" => loopal_tool_api::PermissionMode::Auto,
                _ => loopal_tool_api::PermissionMode::Supervised,
            };
        }
        if self.no_sandbox {
            settings.sandbox.policy = loopal_config::SandboxPolicy::Disabled;
        }
    }
}
