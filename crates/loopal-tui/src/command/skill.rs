//! Skill-based command handler — wraps a `.md` skill template as a CommandHandler.

use async_trait::async_trait;
use loopal_config::Skill;

use super::{CommandEffect, CommandHandler};
use crate::app::App;

/// A command handler backed by a skill template file.
pub struct SkillHandler {
    name: String,
    description: String,
    has_arg: bool,
    body: String,
}

impl SkillHandler {
    pub fn from_skill(skill: &Skill) -> Self {
        Self {
            name: skill.name.clone(),
            description: skill.description.clone(),
            has_arg: skill.has_arg,
            body: skill.body.clone(),
        }
    }
}

#[async_trait]
impl CommandHandler for SkillHandler {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn has_arg(&self) -> bool {
        self.has_arg
    }
    fn is_skill(&self) -> bool {
        true
    }
    fn skill_body(&self) -> Option<&str> {
        Some(&self.body)
    }
    async fn execute(&self, _app: &mut App, arg: Option<&str>) -> CommandEffect {
        let expanded = expand_skill(&self.body, arg.unwrap_or(""));
        CommandEffect::InboxPush(expanded.into())
    }
}

/// Expand a skill template by replacing `$ARGUMENTS` with the given args.
pub fn expand_skill(body: &str, args: &str) -> String {
    let trimmed = args.trim();
    if body.contains("$ARGUMENTS") {
        body.replace("$ARGUMENTS", trimmed)
    } else if !trimmed.is_empty() {
        format!("{body}\n{trimmed}")
    } else {
        body.to_string()
    }
}
