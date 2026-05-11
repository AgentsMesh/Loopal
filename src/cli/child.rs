use std::ffi::OsString;

use clap::Args;

#[derive(Args, Debug, Default, PartialEq, Eq, Clone)]
pub struct ChildPassthroughArgs {
    #[arg(short, long)]
    pub model: Option<String>,

    #[arg(short = 'P', long)]
    pub permission: Option<String>,

    #[arg(long)]
    pub decision: Option<String>,

    #[arg(long)]
    pub plan: bool,

    #[arg(long)]
    pub no_sandbox: bool,

    #[arg(long)]
    pub ephemeral: bool,

    #[arg(long)]
    pub join_hub: Option<String>,

    #[arg(long)]
    pub hub_name: Option<String>,
}

impl ChildPassthroughArgs {
    pub fn to_args(&self) -> Vec<OsString> {
        let mut out = Vec::new();
        if let Some(model) = &self.model {
            out.push("--model".into());
            out.push(model.into());
        }
        if let Some(perm) = &self.permission {
            out.push("--permission".into());
            out.push(perm.into());
        }
        if let Some(decision) = &self.decision {
            out.push("--decision".into());
            out.push(decision.into());
        }
        if self.plan {
            out.push("--plan".into());
        }
        if self.no_sandbox {
            out.push("--no-sandbox".into());
        }
        if self.ephemeral {
            out.push("--ephemeral".into());
        }
        if let Some(addr) = &self.join_hub {
            out.push("--join-hub".into());
            out.push(addr.into());
        }
        if let Some(name) = &self.hub_name {
            out.push("--hub-name".into());
            out.push(name.into());
        }
        out
    }
}
