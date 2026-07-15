//! Skill-based command handler — wraps a `.md` skill template as a CommandHandler.

use async_trait::async_trait;
use loopal_config::{Skill, expand_skill};
use loopal_protocol::SkillInvocation;

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
        let args_str = arg.unwrap_or("");
        let expanded = expand_skill(&self.body, args_str);
        let mut content = loopal_protocol::UserContent::from(expanded);
        content.skill_info = Some(SkillInvocation {
            name: self.name.clone(),
            user_args: args_str.trim().to_string(),
        });
        CommandEffect::InboxPush(content)
    }
}
