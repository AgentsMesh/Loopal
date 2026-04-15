//! Key handler for the status page sub-page.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::InputAction;
use crate::app::{App, StatusTab, SubPage};

/// Handle keys when the status page sub-page is active.
pub(super) fn handle_status_page_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let state = match app.sub_page.as_mut() {
        Some(SubPage::StatusPage(s)) => s,
        _ => return InputAction::None,
    };

    match key.code {
        KeyCode::Esc => {
            app.sub_page = None;
            app.last_esc_time = None;
            InputAction::None
        }
        KeyCode::Left => {
            state.active_tab = state.active_tab.prev();
            InputAction::None
        }
        KeyCode::Right => {
            state.active_tab = state.active_tab.next();
            InputAction::None
        }
        KeyCode::Up => {
            let scroll = state.active_scroll_mut();
            *scroll = scroll.saturating_sub(1);
            InputAction::None
        }
        KeyCode::Down => {
            let max = state.active_row_count().saturating_sub(1);
            let scroll = state.active_scroll_mut();
            if *scroll < max {
                *scroll += 1;
            }
            InputAction::None
        }
        KeyCode::Char(c)
            if state.active_tab == StatusTab::Config
                && !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            state.filter.insert(state.filter_cursor, c);
            state.filter_cursor += c.len_utf8();
            state.scroll_offsets[StatusTab::Config.index()] = 0;
            InputAction::None
        }
        KeyCode::Backspace if state.active_tab == StatusTab::Config => {
            if state.filter_cursor > 0 {
                let prev = state.filter[..state.filter_cursor]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                state.filter.remove(prev);
                state.filter_cursor = prev;
                state.scroll_offsets[StatusTab::Config.index()] = 0;
            }
            InputAction::None
        }
        _ => InputAction::None,
    }
}
