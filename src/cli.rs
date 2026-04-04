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

    /// Initial prompt (non-interactive)
    pub prompt: Vec<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cli_with_resume(resume: Option<String>) -> Cli {
        Cli {
            model: None,
            resume,
            permission: None,
            plan: false,
            no_sandbox: false,
            acp: false,
            server: false,
            ephemeral: false,
            serve: false,
            worktree: false,
            test_provider: None,
            meta_hub: None,
            join_hub: None,
            hub_name: None,
            prompt: vec![],
        }
    }

    #[test]
    fn test_resume_intent_none_when_no_flag() {
        let cli = cli_with_resume(None);
        assert!(cli.resume_intent().is_none());
    }

    #[test]
    fn test_resume_intent_latest_when_empty_string() {
        let cli = cli_with_resume(Some(String::new()));
        let intent = cli.resume_intent().expect("should be Some");
        assert!(matches!(intent, ResumeIntent::Latest));
    }

    #[test]
    fn test_resume_intent_specific_when_id_given() {
        let cli = cli_with_resume(Some("abc-123".into()));
        let intent = cli.resume_intent().expect("should be Some");
        if let ResumeIntent::Specific(id) = intent {
            assert_eq!(id, "abc-123");
        } else {
            panic!("expected ResumeIntent::Specific");
        }
    }
}
