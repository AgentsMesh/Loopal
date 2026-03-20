mod actions;
mod autocomplete;
mod commands;
mod sub_page;

pub use actions::*;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use autocomplete::{handle_autocomplete_key, update_autocomplete};
use commands::try_execute_slash_command;
use sub_page::handle_sub_page_key;

/// Process a key event and update the app's input state.
pub fn handle_key(app: &mut App, key: KeyEvent) -> InputAction {
    // Handle tool confirm state (derived from session pending_permission)
    if app.session.lock().pending_permission.is_some() {
        return match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => InputAction::ToolApprove,
            KeyCode::Char('n') | KeyCode::Char('N') => InputAction::ToolDeny,
            KeyCode::Esc => InputAction::ToolDeny,
            _ => InputAction::None,
        };
    }

    // Handle question dialog state (AskUser tool)
    if app.session.lock().pending_question.is_some() {
        return match key.code {
            KeyCode::Up => InputAction::QuestionUp,
            KeyCode::Down => InputAction::QuestionDown,
            KeyCode::Enter => InputAction::QuestionConfirm,
            KeyCode::Char(' ') => InputAction::QuestionToggle,
            KeyCode::Esc => InputAction::QuestionCancel,
            _ => InputAction::None,
        };
    }

    // --- Sub-page interception (highest priority after tool confirm) ---
    if app.sub_page.is_some() {
        return handle_sub_page_key(app, &key);
    }

    // Ctrl+C / Ctrl+D to quit
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') | KeyCode::Char('d') => return InputAction::Quit,
            _ => {}
        }
    }

    // Shift+Tab to toggle Plan/Act mode (never intercepted by autocomplete)
    if key.code == KeyCode::BackTab {
        let current_mode = app.session.lock().mode.clone();
        let new_mode = if current_mode == "plan" { "act" } else { "plan" };
        return InputAction::ModeSwitch(new_mode.to_string());
    }

    // --- Autocomplete interception ---
    if app.autocomplete.is_some()
        && let Some(action) = handle_autocomplete_key(app, &key)
    {
        return action;
    }

    // --- Normal key handling ---
    let action = match key.code {
        KeyCode::Enter => {
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
            if let Some(text) = app.submit_input() {
                return InputAction::InboxPush(text);
            }
            InputAction::None
        }
        KeyCode::Char(c) => {
            app.input.insert(app.input_cursor, c);
            app.input_cursor += c.len_utf8();
            InputAction::None
        }
        KeyCode::Backspace => {
            if app.input_cursor > 0 {
                let prev = app.input[..app.input_cursor]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                app.input.remove(prev);
                app.input_cursor = prev;
            }
            InputAction::None
        }
        KeyCode::Delete => {
            if app.input_cursor < app.input.len() {
                app.input.remove(app.input_cursor);
            }
            InputAction::None
        }
        KeyCode::Left => {
            if app.input_cursor > 0 {
                app.input_cursor = app.input[..app.input_cursor]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
            }
            InputAction::None
        }
        KeyCode::Right => {
            if app.input_cursor < app.input.len() {
                app.input_cursor = app.input[app.input_cursor..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| app.input_cursor + i)
                    .unwrap_or(app.input.len());
            }
            InputAction::None
        }
        KeyCode::Home => {
            app.input_cursor = 0;
            InputAction::None
        }
        KeyCode::End => {
            app.input_cursor = app.input.len();
            InputAction::None
        }
        KeyCode::Up => {
            if app.pop_inbox_to_input() {
                // Popped last Inbox message back into input for editing
            } else if !app.input_history.is_empty() {
                let idx = match app.history_index {
                    None => app.input_history.len() - 1,
                    Some(i) if i > 0 => i - 1,
                    Some(i) => i,
                };
                app.history_index = Some(idx);
                app.input = app.input_history[idx].clone();
                app.input_cursor = app.input.len();
            }
            InputAction::None
        }
        KeyCode::Down => {
            if let Some(idx) = app.history_index {
                if idx + 1 < app.input_history.len() {
                    let new_idx = idx + 1;
                    app.history_index = Some(new_idx);
                    app.input = app.input_history[new_idx].clone();
                    app.input_cursor = app.input.len();
                } else {
                    app.history_index = None;
                    app.input.clear();
                    app.input_cursor = 0;
                }
            }
            InputAction::None
        }
        KeyCode::Esc => InputAction::None,
        KeyCode::PageUp => {
            app.scroll_offset = app.scroll_offset.saturating_add(10);
            InputAction::None
        }
        KeyCode::PageDown => {
            app.scroll_offset = app.scroll_offset.saturating_sub(10);
            InputAction::None
        }
        _ => InputAction::None,
    };

    update_autocomplete(app);
    action
}
