//! Key handler for the skills sub-page.

use crossterm::event::{KeyCode, KeyEvent};

use super::InputAction;
use crate::app::{App, SubPage};

pub(super) fn handle_skills_page_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let state = match app.sub_page.as_mut() {
        Some(SubPage::SkillsPage(s)) => s,
        _ => return InputAction::None,
    };

    match key.code {
        KeyCode::Esc => {
            app.sub_page = None;
            app.last_esc_time = None;
            InputAction::None
        }
        KeyCode::Up => {
            state.selected = state.selected.saturating_sub(1);
            InputAction::None
        }
        KeyCode::Down => {
            if !state.skills.is_empty() && state.selected + 1 < state.skills.len() {
                state.selected += 1;
            }
            InputAction::None
        }
        _ => InputAction::None,
    }
}
