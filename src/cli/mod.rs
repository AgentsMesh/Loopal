mod child;
mod parent_only;

pub use child::ChildPassthroughArgs;
pub use parent_only::ParentOnlyArgs;

use clap::Parser;

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

    pub prompt: Vec<String>,
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
            settings.permission_mode = match perm.as_str() {
                "bypass" | "yolo" => loopal_tool_api::PermissionMode::Bypass,
                "auto" => loopal_tool_api::PermissionMode::Auto,
                _ => loopal_tool_api::PermissionMode::Supervised,
            };
        }
        if self.child.no_sandbox {
            settings.sandbox.policy = loopal_config::SandboxPolicy::Disabled;
        }
    }
}

#[cfg(test)]
mod apply_overrides_test;
#[cfg(test)]
mod child_args_test;
#[cfg(test)]
mod resume_intent_test;
