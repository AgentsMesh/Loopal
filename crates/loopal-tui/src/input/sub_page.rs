use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, SubPage};

use super::{InputAction, SlashCommandAction};

/// Handle keys when a sub-page (picker) is active. All keys are consumed.
pub(super) fn handle_sub_page_key(app: &mut App, key: &KeyEvent) -> InputAction {
    // Ctrl+C still quits even in sub-page
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') | KeyCode::Char('d') => return InputAction::Quit,
            _ => {}
        }
    }

    let sub_page = app.sub_page.as_mut().unwrap();
    match sub_page {
        SubPage::ModelPicker(picker) => match key.code {
            KeyCode::Esc => {
                app.sub_page = None;
                InputAction::None
            }
            KeyCode::Up => {
                picker.selected = picker.selected.saturating_sub(1);
                InputAction::None
            }
            KeyCode::Down => {
                let count = picker.filtered_items().len();
                if picker.selected + 1 < count {
                    picker.selected += 1;
                }
                InputAction::None
            }
            KeyCode::Enter => {
                let filtered = picker.filtered_items();
                if let Some(item) = filtered.get(picker.selected) {
                    let value = item.value.clone();
                    app.sub_page = None;
                    InputAction::SlashCommand(SlashCommandAction::ModelSelected(value))
                } else {
                    // No items — just close
                    app.sub_page = None;
                    InputAction::None
                }
            }
            KeyCode::Char(c) => {
                picker.filter.insert(picker.filter_cursor, c);
                picker.filter_cursor += c.len_utf8();
                picker.selected = 0;
                picker.clamp_selected();
                InputAction::None
            }
            KeyCode::Backspace => {
                if picker.filter_cursor > 0 {
                    let prev = picker.filter[..picker.filter_cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    picker.filter.remove(prev);
                    picker.filter_cursor = prev;
                    picker.selected = 0;
                    picker.clamp_selected();
                }
                InputAction::None
            }
            _ => InputAction::None,
        },
    }
}
