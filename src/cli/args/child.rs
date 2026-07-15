use std::ffi::OsString;

use clap::Args;
use clap::builder::PossibleValuesParser;

#[derive(Args, Debug, Default, PartialEq, Eq, Clone)]
pub struct ChildPassthroughArgs {
    /// Override default LLM model id (e.g. claude-opus-4-7, gpt-5.5)
    #[arg(short, long, value_name = "MODEL")]
    pub model: Option<String>,

    /// When to ask for permission (alias 'yolo' = bypass)
    #[arg(
        short = 'P',
        long,
        value_name = "MODE",
        value_parser = PossibleValuesParser::new(["bypass", "ask_dangerous", "ask_any_write", "yolo"]),
    )]
    pub permission: Option<String>,

    /// Who answers permission prompts when --permission is not 'bypass'
    #[arg(
        long,
        value_name = "MODE",
        value_parser = PossibleValuesParser::new(["manual", "classifier", "agent"]),
    )]
    pub decision: Option<String>,

    /// Start session in plan mode (read-only exploration first)
    #[arg(long)]
    pub plan: bool,

    /// Disable OS sandbox for tool execution
    #[arg(long)]
    pub no_sandbox: bool,

    /// Don't persist session to ~/.loopal/sessions/
    #[arg(long)]
    pub ephemeral: bool,

    /// Hub TCP address to join (e.g. 127.0.0.1:7890)
    #[arg(long, value_name = "ADDR")]
    pub join_hub: Option<String>,

    /// Display name for this agent inside the hub
    #[arg(long, value_name = "NAME")]
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
