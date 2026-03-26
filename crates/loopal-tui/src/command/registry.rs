//! Command registry — stores built-in and skill handlers for unified dispatch.

use std::sync::Arc;

use loopal_config::Skill;

use super::{CommandEntry, CommandHandler};
use crate::command::skill::SkillHandler;

/// Central registry for all slash command handlers.
///
/// Built-in handlers are registered once at startup.
/// Skill handlers are reloaded from disk on each `/` keystroke.
pub struct CommandRegistry {
    builtins: Vec<Arc<dyn CommandHandler>>,
    skills: Vec<Arc<dyn CommandHandler>>,
}

impl CommandRegistry {
    /// Create a new registry and register all built-in commands.
    pub fn new() -> Self {
        let mut reg = Self {
            builtins: Vec::new(),
            skills: Vec::new(),
        };
        super::builtin::register_all(&mut reg);
        reg
    }

    /// Register a built-in command handler.
    pub fn register(&mut self, handler: Arc<dyn CommandHandler>) {
        self.builtins.push(handler);
    }

    /// Find a handler by name. Builtins take priority over skills.
    /// Returns a cloned `Arc` to avoid borrowing `self`.
    pub fn find(&self, name: &str) -> Option<Arc<dyn CommandHandler>> {
        self.builtins
            .iter()
            .chain(self.skills.iter())
            .find(|h| h.name() == name)
            .cloned()
    }

    /// Replace all skill handlers with freshly loaded skills.
    /// Built-in handlers are preserved. Skills that collide with a built-in name are ignored.
    pub fn reload_skills(&mut self, skills: &[Skill]) {
        let builtin_names: std::collections::HashSet<&str> =
            self.builtins.iter().map(|h| h.name()).collect();
        self.skills = skills
            .iter()
            .filter(|s| !builtin_names.contains(s.name.as_str()))
            .map(|s| Arc::new(SkillHandler::from_skill(s)) as Arc<dyn CommandHandler>)
            .collect();
    }

    /// Generate lightweight entries for autocomplete display.
    pub fn entries(&self) -> Vec<CommandEntry> {
        self.builtins
            .iter()
            .chain(self.skills.iter())
            .map(|h| CommandEntry {
                name: h.name().to_string(),
                description: h.description().to_string(),
                has_arg: h.has_arg(),
                is_skill: h.is_skill(),
            })
            .collect()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
