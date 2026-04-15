//! Key handler for the background task log viewer sub-page.

use crossterm::event::{KeyCode, KeyEvent};

use super::InputAction;
use crate::app::{App, SubPage};

pub(super) fn handle_bg_task_log_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let state = match app.sub_page.as_mut() {
        Some(SubPage::BgTaskLog(s)) => s,
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
            state.auto_follow = false;
            InputAction::None
        }
        KeyCode::Down => {
            state.scroll_offset += 1;
            InputAction::None
        }
        KeyCode::PageUp => {
            state.scroll_offset = state.scroll_offset.saturating_sub(20);
            state.auto_follow = false;
            InputAction::None
        }
        KeyCode::PageDown => {
            state.scroll_offset += 20;
            InputAction::None
        }
        KeyCode::Char('f') | KeyCode::Char('F') => {
            state.auto_follow = !state.auto_follow;
            InputAction::None
        }
        _ => InputAction::None,
    }
}
