//! `/skills` command — lists loaded skills and their sources.

use std::collections::BTreeSet;

use async_trait::async_trait;

use super::{CommandEffect, CommandHandler};
use crate::app::App;

pub struct SkillsCmd;

#[async_trait]
impl CommandHandler for SkillsCmd {
    fn name(&self) -> &str {
        "/skills"
    }
    fn description(&self) -> &str {
        "List loaded skills and sources"
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        let content = build_skills_list(app);
        app.session.push_system_message(content);
        CommandEffect::Done
    }
}

fn build_skills_list(app: &App) -> String {
    let config = match loopal_config::load_config(&app.cwd) {
        Ok(c) => c,
        Err(_) => return "Failed to load config.".to_string(),
    };

    if config.skills.is_empty() {
        return "No skills loaded.".to_string();
    }

    let name_width = config
        .skills
        .values()
        .map(|e| e.skill.name.len())
        .max()
        .unwrap_or(8);

    let mut lines = Vec::with_capacity(config.skills.len() + 3);
    lines.push(format!("Loaded skills ({}):", config.skills.len()));

    let mut sources = BTreeSet::new();
    for entry in config.skills.values() {
        let s = &entry.skill;
        sources.insert(entry.source.to_string());
        lines.push(format!(
            "  {:<width$}  [{}]  {}",
            s.name,
            entry.source,
            s.description,
            width = name_width,
        ));
    }

    lines.push(String::new());
    let legend: Vec<_> = sources.into_iter().collect();
    lines.push(format!("Sources: {}", legend.join(", ")));
    lines.join("\n")
}
