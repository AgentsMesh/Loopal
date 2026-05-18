use crossterm::event::{KeyCode, KeyEvent};

use super::InputAction;
use crate::app::{App, SubPage};

pub(super) fn handle_task_detail_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let state = match app.sub_page.as_mut() {
        Some(SubPage::TaskDetail(s)) => s,
        _ => return InputAction::None,
    };
    match key.code {
        KeyCode::Esc => {
            app.sub_page = None;
            app.last_esc_time = None;
            InputAction::None
        }
        KeyCode::Up => {
            state.scroll_offset = state.scroll_offset.saturating_sub(1);
            InputAction::None
        }
        KeyCode::Down => {
            state.scroll_offset += 1;
            InputAction::None
        }
        KeyCode::PageUp => {
            state.scroll_offset = state.scroll_offset.saturating_sub(10);
            InputAction::None
        }
        KeyCode::PageDown => {
            state.scroll_offset += 10;
            InputAction::None
        }
        _ => InputAction::None,
    }
}
