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
        SubPage::ModelPicker(_) => handle_model_picker_key(app, key),
        SubPage::RewindPicker(_) => handle_rewind_picker_key(app, key),
    }
}

fn handle_model_picker_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let picker = match app.sub_page.as_mut().unwrap() {
        SubPage::ModelPicker(p) => p,
        _ => unreachable!(),
    };
    match key.code {
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
                let model = item.value.clone();
                let thinking_json = picker
                    .thinking_options
                    .get(picker.thinking_selected)
                    .map(|o| o.value.clone());
                app.sub_page = None;
                match thinking_json {
                    Some(json) => InputAction::SlashCommand(
                        SlashCommandAction::ModelAndThinkingSelected {
                            model,
                            thinking_json: json,
                        },
                    ),
                    None => InputAction::SlashCommand(
                        SlashCommandAction::ModelSelected(model),
                    ),
                }
            } else {
                app.sub_page = None;
                InputAction::None
            }
        }
        KeyCode::Left => {
            if !picker.thinking_options.is_empty() {
                picker.thinking_selected = if picker.thinking_selected == 0 {
                    picker.thinking_options.len() - 1
                } else {
                    picker.thinking_selected - 1
                };
            }
            InputAction::None
        }
        KeyCode::Right => {
            if !picker.thinking_options.is_empty() {
                picker.thinking_selected =
                    (picker.thinking_selected + 1) % picker.thinking_options.len();
            }
            InputAction::None
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
    }
}

fn handle_rewind_picker_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let state = match app.sub_page.as_mut().unwrap() {
        SubPage::RewindPicker(s) => s,
        _ => unreachable!(),
    };
    match key.code {
        KeyCode::Esc => {
            app.sub_page = None;
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
                InputAction::SlashCommand(SlashCommandAction::RewindConfirmed(turn_index))
            } else {
                app.sub_page = None;
                InputAction::None
            }
        }
        _ => InputAction::None,
    }
}
