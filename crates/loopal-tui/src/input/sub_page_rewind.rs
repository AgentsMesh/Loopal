//! Rewind picker key handling (sub-page).

use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, SubPage};

use super::{InputAction, SubPageResult};

pub(super) fn handle_rewind_picker_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let state = match app.sub_page.as_mut().unwrap() {
        SubPage::RewindPicker(s) => s,
        _ => unreachable!(),
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
            if state.selected + 1 < state.turns.len() {
                state.selected += 1;
            }
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(item) = state.turns.get(state.selected) {
                let turn_index = item.turn_index;
                app.sub_page = None;
                app.last_esc_time = None;
                InputAction::SubPageConfirm(SubPageResult::RewindConfirmed(turn_index))
            } else {
                app.sub_page = None;
                InputAction::None
            }
        }
        _ => InputAction::None,
    }
}
