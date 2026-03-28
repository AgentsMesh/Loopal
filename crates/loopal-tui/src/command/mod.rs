//! Unified slash command system — trait-based handlers with registry.

mod agent_cmd;
mod builtin;
mod help_cmd;
mod init_cmd;
mod model_cmd;
pub mod registry;
mod rewind_cmd;
mod skill;
mod topology_cmd;

use async_trait::async_trait;
use loopal_protocol::{AgentMode, UserContent};

use crate::app::App;

/// Result of executing a slash command.
/// `key_dispatch` maps these to concrete side-effects.
pub enum CommandEffect {
    /// Command completed all work internally (e.g. push_system_message, open SubPage).
    Done,
    /// Push expanded content into the inbox for the agent.
    InboxPush(UserContent),
    /// Switch agent mode (plan / act).
    ModeSwitch(AgentMode),
    /// Exit the application.
    Quit,
}

/// Slash command handler trait.
///
/// Both built-in commands and skills implement this trait and are registered
/// in the [`CommandRegistry`] for unified dispatch.
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Command name including the leading `/`, e.g. "/clear".
    fn name(&self) -> &str;
    /// Short description for autocomplete / help display.
    fn description(&self) -> &str;
    /// Whether the command accepts an argument after the name.
    fn has_arg(&self) -> bool {
        false
    }
    /// Whether this handler originates from a skill file.
    fn is_skill(&self) -> bool {
        false
    }
    /// Return the skill body template (for `/help <skill>` display). `None` for built-in commands.
    fn skill_body(&self) -> Option<&str> {
        None
    }
    /// Execute the command. `arg` is the text after the command name (trimmed).
    async fn execute(&self, app: &mut App, arg: Option<&str>) -> CommandEffect;
}

/// Lightweight entry for autocomplete display and filtering.
#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub name: String,
    pub description: String,
    pub has_arg: bool,
    pub is_skill: bool,
}

pub use registry::CommandRegistry;

/// Filter entries by prefix, returning owned copies of matching entries.
pub fn filter_entries(entries: &[CommandEntry], input: &str) -> Vec<CommandEntry> {
    let lower = input.to_ascii_lowercase();
    entries
        .iter()
        .filter(|e| input == "/" || e.name.starts_with(&lower))
        .cloned()
        .collect()
}
