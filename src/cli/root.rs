use clap::{CommandFactory, Parser};

use super::{ChildPassthroughArgs, ParentOnlyArgs, desktop_command};

#[derive(Debug)]
pub enum ResumeIntent {
    Latest,
    Specific(String),
}

#[derive(Parser)]
#[command(name = "loopal", about = "AI coding agent", version = env!("LOOPAL_VERSION"))]
#[command(group(
    clap::ArgGroup::new("hub_action")
        .args(["list_hubs", "attach_hub_pid", "kill_hub", "attach_hub"])
        .multiple(false)
))]
pub struct Cli {
    #[command(flatten)]
    pub child: ChildPassthroughArgs,

    #[command(flatten)]
    pub parent_only: ParentOnlyArgs,

    /// Prompt for the agent (everything after the flags)
    pub prompt: Vec<String>,
}

pub fn build_cli() -> clap::Command {
    Cli::command()
        .subcommand(desktop_command())
        .subcommand(loopal_vault_age::cli::vault_command())
        .subcommand(loopal_vault_age::cli::vaults_command())
}

impl Cli {
    pub fn resume_intent(&self) -> Option<ResumeIntent> {
        match self.parent_only.resume.as_deref() {
            None => None,
            Some("") => Some(ResumeIntent::Latest),
            Some(id) => Some(ResumeIntent::Specific(id.to_string())),
        }
    }

    pub fn apply_overrides(&self, settings: &mut loopal_config::Settings) {
        if let Some(model) = &self.child.model {
            settings.model = model.clone();
        }
        if let Some(perm) = &self.child.permission {
            let canonical = if perm == "yolo" {
                "bypass"
            } else {
                perm.as_str()
            };
            settings.permission_mode = canonical
                .parse::<loopal_tool_api::PermissionMode>()
                .expect("clap PossibleValuesParser guarantees a known mode");
        }
        if let Some(decision) = &self.child.decision {
            settings.decision_mode = decision
                .parse::<loopal_decision_api::DecisionMode>()
                .expect("clap PossibleValuesParser guarantees a known mode");
        }
        if self.child.no_sandbox {
            settings.sandbox.policy = loopal_config::SandboxPolicy::Disabled;
        }
    }
}
