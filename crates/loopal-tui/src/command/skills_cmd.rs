//! `/skills` command — opens the skills sub-page.

use async_trait::async_trait;

use super::{CommandEffect, CommandHandler};
use crate::app::{App, SkillItem, SkillsPageState, SubPage};

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
        let state = build_skills_page_state(app);
        app.sub_page = Some(SubPage::SkillsPage(state));
        CommandEffect::Done
    }
}

fn build_skills_page_state(app: &App) -> SkillsPageState {
    let config = match loopal_config::load_config(&app.cwd) {
        Ok(c) => c,
        Err(_) => return SkillsPageState::new(Vec::new()),
    };

    let mut items: Vec<SkillItem> = config
        .skills
        .values()
        .map(|entry| SkillItem {
            name: entry.skill.name.clone(),
            source: entry.source.to_string(),
            description: entry.skill.description.clone(),
            has_arg: entry.skill.has_arg,
        })
        .collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    SkillsPageState::new(items)
}
