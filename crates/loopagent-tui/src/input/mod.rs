mod autocomplete;
mod commands;
mod sub_page;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use autocomplete::{handle_autocomplete_key, update_autocomplete};
use commands::try_execute_slash_command;
use sub_page::handle_sub_page_key;

/// Action triggered by a slash command from the autocomplete menu.
pub enum SlashCommandAction {
    Clear,
    Compact,
    Status,
    Sessions,
    Help,
    /// Open the model picker sub-page.
    ModelPicker,
    /// A model was selected from the picker.
    ModelSelected(String),
}

/// Action resulting from input handling
pub enum InputAction {
    /// No action needed
    None,
    /// User submitted a message (legacy — slash commands still use Submit path)
    Submit(String),
    /// User message queued into Inbox (not sent directly to agent)
    InboxPush(String),
    /// User wants to quit
    Quit,
    /// User approved tool use
    ToolApprove,
    /// User denied tool use
    ToolDeny,
    /// User wants to switch mode
    ModeSwitch(String),
    /// User executed a slash command
    SlashCommand(SlashCommandAction),
}

/// Process a key event and update the app's input state.
pub fn handle_key(app: &mut App, key: KeyEvent) -> InputAction {
    // Handle tool confirm state
    if let crate::app::AppState::ToolConfirm { .. } = &app.state {
        return match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => InputAction::ToolApprove,
            KeyCode::Char('n') | KeyCode::Char('N') => InputAction::ToolDeny,
            KeyCode::Esc => InputAction::ToolDeny,
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
        let new_mode = if app.mode == "plan" { "act" } else { "plan" };
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
            // Slash command entered without autocomplete (e.g., typed "/plan" manually)
            let trimmed = app.input.trim().to_string();
            if let Some(cmd_action) = try_execute_slash_command(&trimmed) {
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
                // Inbox empty — fall back to history navigation
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
        KeyCode::Esc => {
            // Close autocomplete if open (handled above), otherwise no-op
            InputAction::None
        }
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

    // After character input / backspace / delete, update autocomplete state
    update_autocomplete(app);

    action
}
