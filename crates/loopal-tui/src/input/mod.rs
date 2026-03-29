mod actions;
mod autocomplete;
mod commands;
mod editing;
pub(crate) mod multiline;
mod navigation;
pub(crate) mod paste;
mod sub_page;

pub use actions::*;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use autocomplete::{handle_autocomplete_key, update_autocomplete};
use editing::{handle_backspace, handle_ctrl_c, handle_enter};
use navigation::{
    DEFAULT_WRAP_WIDTH, handle_down, handle_esc, handle_up, move_cursor_left, move_cursor_right,
};
use sub_page::handle_sub_page_key;

/// Process a key event and update the app's input state.
pub fn handle_key(app: &mut App, key: KeyEvent) -> InputAction {
    if let Some(action) = handle_modal_keys(app, &key) {
        return action;
    }
    if let Some(action) = handle_global_keys(app, &key) {
        return action;
    }
    if app.autocomplete.is_some()
        && let Some(action) = handle_autocomplete_key(app, &key)
    {
        return action;
    }

    let action = handle_normal_key(app, &key);
    update_autocomplete(app);
    action
}

/// Handle modal states: tool confirm, question dialog, sub-page.
fn handle_modal_keys(app: &mut App, key: &KeyEvent) -> Option<InputAction> {
    if app.session.lock().pending_permission.is_some() {
        let is_ctrl_c =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
        return Some(match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => InputAction::ToolApprove,
            KeyCode::Char('n') | KeyCode::Char('N') => InputAction::ToolDeny,
            KeyCode::Esc => InputAction::ToolDeny,
            _ if is_ctrl_c => InputAction::ToolDeny,
            _ => InputAction::None,
        });
    }
    if app.session.lock().pending_question.is_some() {
        let is_ctrl_c =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
        return Some(match key.code {
            KeyCode::Up => InputAction::QuestionUp,
            KeyCode::Down => InputAction::QuestionDown,
            KeyCode::Enter => InputAction::QuestionConfirm,
            KeyCode::Char(' ') => InputAction::QuestionToggle,
            KeyCode::Esc => InputAction::QuestionCancel,
            _ if is_ctrl_c => InputAction::QuestionCancel,
            _ => InputAction::None,
        });
    }
    if app.sub_page.is_some() {
        return Some(handle_sub_page_key(app, key));
    }
    None
}

/// Handle global shortcuts: Ctrl combos, Shift+Tab.
fn handle_global_keys(app: &mut App, key: &KeyEvent) -> Option<InputAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') => return Some(handle_ctrl_c(app)),
            KeyCode::Char('d') => return Some(InputAction::Quit),
            KeyCode::Char('v') => return Some(InputAction::PasteRequested),
            // Readline-style history: Ctrl+P/N always navigate history,
            // independent of scroll state. This avoids the conflict between
            // xterm alternate scroll (mouse wheel → Up/Down) and history.
            KeyCode::Char('p') => return Some(handle_up(app)),
            KeyCode::Char('n') => return Some(handle_down(app)),
            _ => {}
        }
    }
    if key.code == KeyCode::BackTab {
        let current_mode = app.session.lock().mode.clone();
        let new_mode = if current_mode == "plan" {
            "act"
        } else {
            "plan"
        };
        return Some(InputAction::ModeSwitch(new_mode.to_string()));
    }
    None
}

/// Handle normal input keys (typing, navigation, submit).
fn handle_normal_key(app: &mut App, key: &KeyEvent) -> InputAction {
    match key.code {
        // Shift+Enter → insert newline (must be before Enter)
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.input.insert(app.input_cursor, '\n');
            app.input_cursor += 1;
            InputAction::None
        }
        KeyCode::Enter => handle_enter(app),
        KeyCode::Char(c) => {
            app.input.insert(app.input_cursor, c);
            app.input_cursor += c.len_utf8();
            InputAction::None
        }
        KeyCode::Backspace => handle_backspace(app),
        KeyCode::Delete => {
            if app.input_cursor < app.input.len() {
                app.input.remove(app.input_cursor);
            }
            InputAction::None
        }
        KeyCode::Left => {
            move_cursor_left(app);
            InputAction::None
        }
        KeyCode::Right => {
            move_cursor_right(app);
            InputAction::None
        }
        KeyCode::Home => {
            app.input_cursor =
                multiline::line_home(&app.input, app.input_cursor, DEFAULT_WRAP_WIDTH);
            InputAction::None
        }
        KeyCode::End => {
            app.input_cursor =
                multiline::line_end(&app.input, app.input_cursor, DEFAULT_WRAP_WIDTH);
            InputAction::None
        }
        // Up/Down priority chain:
        //   1. Multiline cursor navigation (Shift+Enter input)
        //   2. Content scroll (mouse wheel via xterm alternate scroll)
        //   3. History navigation (only when content fits on screen)
        // Ctrl+P/N always navigate history regardless of scroll state.
        KeyCode::Up => {
            if multiline::is_multiline(&app.input, DEFAULT_WRAP_WIDTH)
                && let Some(pos) =
                    multiline::cursor_up(&app.input, app.input_cursor, DEFAULT_WRAP_WIDTH)
            {
                app.input_cursor = pos;
                InputAction::None
            } else if app.scroll_offset > 0
                || !app.session.lock().agent_idle
                || app.content_overflows
            {
                app.scroll_offset = app.scroll_offset.saturating_add(1);
                InputAction::None
            } else {
                handle_up(app)
            }
        }
        KeyCode::Down => {
            if multiline::is_multiline(&app.input, DEFAULT_WRAP_WIDTH)
                && let Some(pos) =
                    multiline::cursor_down(&app.input, app.input_cursor, DEFAULT_WRAP_WIDTH)
            {
                app.input_cursor = pos;
                InputAction::None
            } else if app.scroll_offset > 0 {
                app.scroll_offset = app.scroll_offset.saturating_sub(1);
                InputAction::None
            } else if app.content_overflows {
                InputAction::None
            } else {
                handle_down(app)
            }
        }
        KeyCode::Esc => handle_esc(app),
        KeyCode::PageUp => {
            app.scroll_offset = app.scroll_offset.saturating_add(10);
            InputAction::None
        }
        KeyCode::PageDown => {
            app.scroll_offset = app.scroll_offset.saturating_sub(10);
            InputAction::None
        }
        _ => InputAction::None,
    }
}
