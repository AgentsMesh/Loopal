use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, PickerState, SubPage};

use super::status_page_keys::handle_status_page_key;
use super::sub_page_rewind::handle_rewind_picker_key;
use super::{InputAction, SubPageResult};

/// Handle keys when a sub-page (picker) is active. All keys are consumed.
pub(super) fn handle_sub_page_key(app: &mut App, key: &KeyEvent) -> InputAction {
    // Ctrl+C closes sub-page; Ctrl+D quits
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') => {
                app.sub_page = None;
                app.last_esc_time = None;
                return InputAction::None;
            }
            KeyCode::Char('d') => return InputAction::Quit,
            _ => {}
        }
    }

    let sub_page = app.sub_page.as_mut().unwrap();
    match sub_page {
        SubPage::ModelPicker(_) => handle_model_picker_key(app, key),
        SubPage::RewindPicker(_) => handle_rewind_picker_key(app, key),
        SubPage::SessionPicker(_) => handle_session_picker_key(app, key),
        SubPage::StatusPage(_) => handle_status_page_key(app, key),
    }
}

// ── Generic picker navigation (Esc/Up/Down/Char/Backspace) ────────

/// Result of generic picker key handling.
enum PickerKeyResult {
    /// Picker should be dismissed (Esc pressed).
    Dismiss,
    /// Key was handled (navigation / filter edit).
    Handled,
    /// Key not handled — caller should process (Enter, Left/Right, etc.).
    Unhandled,
}

/// Handle common PickerState keys. Does NOT touch `app.sub_page`.
fn handle_generic_picker_key(picker: &mut PickerState, key: &KeyEvent) -> PickerKeyResult {
    match key.code {
        KeyCode::Esc => PickerKeyResult::Dismiss,
        KeyCode::Up => {
            picker.selected = picker.selected.saturating_sub(1);
            PickerKeyResult::Handled
        }
        KeyCode::Down => {
            let count = picker.filtered_items().len();
            if picker.selected + 1 < count {
                picker.selected += 1;
            }
            PickerKeyResult::Handled
        }
        KeyCode::Char(c) => {
            picker.filter.insert(picker.filter_cursor, c);
            picker.filter_cursor += c.len_utf8();
            picker.selected = 0;
            picker.clamp_selected();
            PickerKeyResult::Handled
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
            PickerKeyResult::Handled
        }
        _ => PickerKeyResult::Unhandled,
    }
}

/// Dismiss the picker overlay and reset ESC state.
fn dismiss_picker(app: &mut App) {
    app.sub_page = None;
    app.last_esc_time = None;
}

// ── Model picker ──────────────────────────────────────────────────

fn handle_model_picker_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let picker = match app.sub_page.as_mut().unwrap() {
        SubPage::ModelPicker(p) => p,
        _ => unreachable!(),
    };
    match handle_generic_picker_key(picker, key) {
        PickerKeyResult::Dismiss => {
            dismiss_picker(app);
            return InputAction::None;
        }
        PickerKeyResult::Handled => return InputAction::None,
        PickerKeyResult::Unhandled => {}
    }
    let picker = match app.sub_page.as_mut().unwrap() {
        SubPage::ModelPicker(p) => p,
        _ => unreachable!(),
    };
    match key.code {
        KeyCode::Enter => {
            let filtered = picker.filtered_items();
            if let Some(item) = filtered.get(picker.selected) {
                let model = item.value.clone();
                let thinking_json = picker
                    .thinking_options
                    .get(picker.thinking_selected)
                    .map(|o| o.value.clone());
                dismiss_picker(app);
                match thinking_json {
                    Some(json) => {
                        InputAction::SubPageConfirm(SubPageResult::ModelAndThinkingSelected {
                            model,
                            thinking_json: json,
                        })
                    }
                    None => InputAction::SubPageConfirm(SubPageResult::ModelSelected(model)),
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
        _ => InputAction::None,
    }
}

// ── Session picker ────────────────────────────────────────────────

fn handle_session_picker_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let picker = match app.sub_page.as_mut().unwrap() {
        SubPage::SessionPicker(p) => p,
        _ => unreachable!(),
    };
    match handle_generic_picker_key(picker, key) {
        PickerKeyResult::Dismiss => {
            dismiss_picker(app);
            return InputAction::None;
        }
        PickerKeyResult::Handled => return InputAction::None,
        PickerKeyResult::Unhandled => {}
    }
    let picker = match app.sub_page.as_mut().unwrap() {
        SubPage::SessionPicker(p) => p,
        _ => unreachable!(),
    };
    if key.code == KeyCode::Enter {
        let filtered = picker.filtered_items();
        if let Some(item) = filtered.get(picker.selected) {
            let session_id = item.value.clone();
            dismiss_picker(app);
            InputAction::SubPageConfirm(SubPageResult::SessionSelected(session_id))
        } else {
            app.sub_page = None;
            InputAction::None
        }
    } else {
        InputAction::None
    }
}
