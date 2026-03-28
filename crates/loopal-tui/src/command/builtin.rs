//! Simple built-in command handlers.
//!
//! Commands with substantial logic (model, rewind, init, help) live in their own modules.

use std::sync::Arc;

use async_trait::async_trait;
use loopal_protocol::AgentMode;

use super::{CommandEffect, CommandHandler};
use crate::app::App;
use crate::command::registry::CommandRegistry;

// ---------------------------------------------------------------------------
// Trivial commands
// ---------------------------------------------------------------------------

pub struct ClearCmd;

#[async_trait]
impl CommandHandler for ClearCmd {
    fn name(&self) -> &str {
        "/clear"
    }
    fn description(&self) -> &str {
        "Clear conversation history"
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        app.pending_images.clear();
        app.session.clear().await;
        CommandEffect::Done
    }
}

pub struct CompactCmd;

#[async_trait]
impl CommandHandler for CompactCmd {
    fn name(&self) -> &str {
        "/compact"
    }
    fn description(&self) -> &str {
        "Compact old messages"
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        app.session.compact().await;
        CommandEffect::Done
    }
}

pub struct StatusCmd;

#[async_trait]
impl CommandHandler for StatusCmd {
    fn name(&self) -> &str {
        "/status"
    }
    fn description(&self) -> &str {
        "Show current status"
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        let state = app.session.lock();
        let token_count = state.token_count();
        let context_info = if state.context_window > 0 {
            format!("{}k/{}k", token_count / 1000, state.context_window / 1000)
        } else {
            format!("{token_count} tokens")
        };
        let status = format!(
            "Mode: {} | Model: {} | Context: {} | Turns: {} | CWD: {}",
            state.mode.to_uppercase(),
            state.model,
            context_info,
            state.turn_count,
            app.cwd.display(),
        );
        drop(state);
        app.session.push_system_message(status);
        CommandEffect::Done
    }
}

pub struct SessionsCmd;

#[async_trait]
impl CommandHandler for SessionsCmd {
    fn name(&self) -> &str {
        "/sessions"
    }
    fn description(&self) -> &str {
        "List session history"
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        app.session
            .push_system_message("Session listing is not yet available in TUI.".to_string());
        CommandEffect::Done
    }
}

pub struct PlanCmd;

#[async_trait]
impl CommandHandler for PlanCmd {
    fn name(&self) -> &str {
        "/plan"
    }
    fn description(&self) -> &str {
        "Switch to Plan mode"
    }
    async fn execute(&self, _app: &mut App, _arg: Option<&str>) -> CommandEffect {
        CommandEffect::ModeSwitch(AgentMode::Plan)
    }
}

pub struct ActCmd;

#[async_trait]
impl CommandHandler for ActCmd {
    fn name(&self) -> &str {
        "/act"
    }
    fn description(&self) -> &str {
        "Switch to Act mode"
    }
    async fn execute(&self, _app: &mut App, _arg: Option<&str>) -> CommandEffect {
        CommandEffect::ModeSwitch(AgentMode::Act)
    }
}

pub struct ExitCmd;

#[async_trait]
impl CommandHandler for ExitCmd {
    fn name(&self) -> &str {
        "/exit"
    }
    fn description(&self) -> &str {
        "Exit the application"
    }
    async fn execute(&self, _app: &mut App, _arg: Option<&str>) -> CommandEffect {
        CommandEffect::Quit
    }
}

/// Register all built-in command handlers.
pub fn register_all(registry: &mut CommandRegistry) {
    registry.register(Arc::new(PlanCmd));
    registry.register(Arc::new(ActCmd));
    registry.register(Arc::new(ClearCmd));
    registry.register(Arc::new(CompactCmd));
    registry.register(Arc::new(super::model_cmd::ModelCmd));
    registry.register(Arc::new(super::rewind_cmd::RewindCmd));
    registry.register(Arc::new(StatusCmd));
    registry.register(Arc::new(SessionsCmd));
    registry.register(Arc::new(super::init_cmd::InitCmd));
    registry.register(Arc::new(super::help_cmd::HelpCmd));
    registry.register(Arc::new(ExitCmd));
    registry.register(Arc::new(super::agent_cmd::AgentsCmd));
}
