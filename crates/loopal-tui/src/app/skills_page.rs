//! State for the `/skills` sub-page — skill list with source info.

/// A single skill entry for display.
pub struct SkillItem {
    pub name: String,
    pub source: String,
    pub description: String,
    pub has_arg: bool,
}

/// Full state for the skills sub-page.
pub struct SkillsPageState {
    pub skills: Vec<SkillItem>,
    pub selected: usize,
    pub scroll_offset: usize,
}

impl SkillsPageState {
    pub fn new(skills: Vec<SkillItem>) -> Self {
        Self {
            skills,
            selected: 0,
            scroll_offset: 0,
        }
    }

    pub fn selected_skill(&self) -> Option<&SkillItem> {
        self.skills.get(self.selected)
    }
}
