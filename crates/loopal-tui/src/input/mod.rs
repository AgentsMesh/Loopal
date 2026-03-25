mod actions;
mod autocomplete;
mod commands;
pub(crate) mod multiline;
mod navigation;
pub(crate) mod paste;
mod sub_page;

pub use actions::*;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use autocomplete::{handle_autocomplete_key, update_autocomplete};
use commands::try_execute_slash_command;
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
        return Some(match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => InputAction::ToolApprove,
            KeyCode::Char('n') | KeyCode::Char('N') => InputAction::ToolDeny,
            KeyCode::Esc => InputAction::ToolDeny,
            _ => InputAction::None,
        });
    }
    if app.session.lock().pending_question.is_some() {
        return Some(match key.code {
            KeyCode::Up => InputAction::QuestionUp,
            KeyCode::Down => InputAction::QuestionDown,
            KeyCode::Enter => InputAction::QuestionConfirm,
            KeyCode::Char(' ') => InputAction::QuestionToggle,
            KeyCode::Esc => InputAction::QuestionCancel,
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
            KeyCode::Char('c') | KeyCode::Char('d') => return Some(InputAction::Quit),
            KeyCode::Char('v') => return Some(InputAction::PasteRequested),
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
        KeyCode::Up if app.scroll_offset > 0 || !app.session.lock().agent_idle => {
            // Browsing mode or agent running: scroll content area.
            app.scroll_offset = app.scroll_offset.saturating_add(1);
            InputAction::None
        }
        KeyCode::Up if app.content_overflows => {
            // Content exceeds viewport: scroll takes priority over history.
            // Alternate scroll sends ~3 Up arrows per wheel notch, so step=1
            // yields ~3 lines/notch — matching prior mouse-capture behavior.
            app.scroll_offset = app.scroll_offset.saturating_add(1);
            InputAction::None
        }
        KeyCode::Down if app.scroll_offset > 0 => {
            app.scroll_offset = app.scroll_offset.saturating_sub(1);
            InputAction::None
        }
        KeyCode::Up => handle_up(app),
        KeyCode::Down => handle_down(app),
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

fn handle_enter(app: &mut App) -> InputAction {
    let trimmed = app.input.trim().to_string();
    if trimmed.starts_with('/') {
        app.refresh_commands();
    }
    if let Some(cmd_action) = try_execute_slash_command(&trimmed, &app.commands) {
        app.input.clear();
        app.input_cursor = 0;
        app.autocomplete = None;
        return cmd_action;
    }
    if let Some(content) = app.submit_input() {
        return InputAction::InboxPush(content);
    }
    InputAction::None
}

fn handle_backspace(app: &mut App) -> InputAction {
    if app.input_cursor > 0 {
        let prev = app.input[..app.input_cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        app.input.remove(prev);
        app.input_cursor = prev;
    } else if !app.pending_images.is_empty() {
        app.pending_images.pop();
    }
    InputAction::None
}
