//! Agent connection commands: /agents.
//!
//! In Hub-only gateway mode, all agents are managed by Hub.
//! Detach/attach commands are no longer needed.

use async_trait::async_trait;

use crate::app::App;
use crate::command::{CommandEffect, CommandHandler};

/// List all agents and their connection status.
pub struct AgentsCmd;

#[async_trait]
impl CommandHandler for AgentsCmd {
    fn name(&self) -> &str {
        "/agents"
    }
    fn description(&self) -> &str {
        "List agents and their connection status"
    }

    fn has_arg(&self) -> bool {
        false
    }

    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        let agents = app.session.list_agents().await;
        if agents.is_empty() {
            return CommandEffect::Reply("No sub-agents".into());
        }
        let lines: Vec<String> = agents
            .iter()
            .map(|(name, state)| format!("  {name}: {state}"))
            .collect();
        CommandEffect::Reply(format!("Agents:\n{}", lines.join("\n")))
    }
}
