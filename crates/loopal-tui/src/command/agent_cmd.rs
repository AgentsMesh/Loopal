//! Agent connection commands: /agents, /detach, /attach.

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

    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        let agents = app.session.list_agents().await;
        if agents.is_empty() {
            app.session.push_system_message("No sub-agents".into());
        } else {
            let lines: Vec<String> = agents
                .iter()
                .map(|(name, state)| format!("  {name}: {state}"))
                .collect();
            app.session
                .push_system_message(format!("Agents:\n{}", lines.join("\n")));
        }
        CommandEffect::Done
    }
}

/// Detach from focused sub-agent (agent keeps running).
pub struct DetachCmd;

#[async_trait]
impl CommandHandler for DetachCmd {
    fn name(&self) -> &str {
        "/detach"
    }
    fn description(&self) -> &str {
        "Detach from focused sub-agent"
    }

    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        let focused = app.session.lock().focused_agent.clone();
        if let Some(name) = focused {
            app.session.detach_agent(&name).await;
            app.session
                .push_system_message(format!("Detached from {name}"));
        } else {
            app.session
                .push_system_message("No focused agent to detach".into());
        }
        CommandEffect::Done
    }
}

/// Re-attach to a detached sub-agent.
pub struct AttachCmd;

#[async_trait]
impl CommandHandler for AttachCmd {
    fn name(&self) -> &str {
        "/attach"
    }
    fn description(&self) -> &str {
        "Re-attach to a detached sub-agent"
    }
    fn has_arg(&self) -> bool {
        true
    }

    async fn execute(&self, app: &mut App, arg: Option<&str>) -> CommandEffect {
        let name = match arg {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
                app.session
                    .push_system_message("Usage: /attach <agent-name>".into());
                return CommandEffect::Done;
            }
        };
        match app.session.reattach_agent(&name).await {
            Ok(()) => {
                app.session
                    .push_system_message(format!("Re-attached to {name}"));
            }
            Err(e) => {
                app.session
                    .push_system_message(format!("Attach failed: {e}"));
            }
        }
        CommandEffect::Done
    }
}
