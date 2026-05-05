use clap::Parser;

/// Parsed resume intent — hides the clap-level empty-string sentinel.
pub enum ResumeIntent {
    /// `--resume` (no ID): auto-find latest session for current directory.
    Latest,
    /// `--resume <ID>`: resume a specific session.
    Specific(String),
}

#[derive(Parser)]
#[command(name = "loopal", about = "AI coding agent", version = "0.1.0")]
#[command(group(
    clap::ArgGroup::new("hub_action")
        .args(["list_hubs", "attach_hub_pid", "kill_hub", "attach_hub"])
        .multiple(false)
))]
pub struct Cli {
    /// Model to use
    #[arg(short, long)]
    pub model: Option<String>,

    /// Resume a previous session (by ID, or latest for current directory if no ID given)
    #[arg(short, long, num_args = 0..=1, default_missing_value = "")]
    resume: Option<String>,

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

    /// Attach to an existing Hub (`addr:port`) as an additional UI
    /// client instead of starting a new Hub. Use the `Hub listening`
    /// stderr line printed by the first instance to find the address.
    #[arg(long)]
    pub attach_hub: Option<String>,

    /// Auth token required by `--attach-hub`. Printed on stderr by the
    /// first instance alongside `Hub listening`.
    #[arg(long)]
    pub hub_token: Option<String>,

    /// Internal: run as standalone Hub process (no TUI). Spawned by the
    /// default `loopal` flow; users do not invoke this directly.
    #[arg(long, hide = true)]
    pub hub_only: bool,

    /// List orphan Hub processes for the current user (PID, port, cwd).
    #[arg(long)]
    pub list_hubs: bool,

    /// Attach to a Hub by PID. Reads the discovery record under
    /// ~/.loopal/run/<pid>.json and obtains the auth token from the
    /// per-pid Unix socket via SO_PEERCRED authentication.
    #[arg(long, value_name = "PID", value_parser = parse_pid)]
    pub attach_hub_pid: Option<u32>,

    /// Shut down a Hub by PID. Same handshake as `--attach-hub-pid`,
    /// but sends `hub/shutdown` instead of opening a TUI.
    #[arg(long, value_name = "PID", value_parser = parse_pid)]
    pub kill_hub: Option<u32>,

    /// Initial prompt (non-interactive)
    pub prompt: Vec<String>,
}

fn parse_pid(s: &str) -> Result<u32, String> {
    let pid: u32 = s.parse().map_err(|e| format!("invalid pid {s:?}: {e}"))?;
    if pid == 0 {
        return Err("pid must be > 0".into());
    }
    Ok(pid)
}

impl Cli {
    /// Parse the raw `--resume` flag into a typed intent.
    /// Encapsulates the clap-level `default_missing_value = ""` convention.
    pub fn resume_intent(&self) -> Option<ResumeIntent> {
        match self.resume.as_deref() {
            None => None,
            Some("") => Some(ResumeIntent::Latest),
            Some(id) => Some(ResumeIntent::Specific(id.to_string())),
        }
    }

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
