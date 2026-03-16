use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AutocompleteState};
use crate::command::filter_commands;

use super::commands::dispatch_command;
use super::InputAction;

/// Handle keys when the autocomplete menu is open.
/// Returns Some(action) if the key was consumed, None to fall through.
pub(super) fn handle_autocomplete_key(app: &mut App, key: &KeyEvent) -> Option<InputAction> {
    match key.code {
        KeyCode::Up => {
            if let Some(ref mut ac) = app.autocomplete {
                ac.selected = ac.selected.saturating_sub(1);
            }
            Some(InputAction::None)
        }
        KeyCode::Down => {
            if let Some(ref mut ac) = app.autocomplete
                && ac.selected + 1 < ac.matches.len()
            {
                ac.selected += 1;
            }
            Some(InputAction::None)
        }
        KeyCode::Tab | KeyCode::Enter => {
            // Confirm the selected command
            let selected_cmd = app
                .autocomplete
                .as_ref()
                .and_then(|ac| ac.matches.get(ac.selected).copied());
            if let Some(cmd) = selected_cmd {
                if cmd.has_arg {
                    // Fill in the command name + space, wait for argument
                    let new_input = format!("{} ", cmd.name);
                    app.input_cursor = new_input.len();
                    app.input = new_input;
                    app.autocomplete = None;
                    Some(InputAction::None)
                } else {
                    // Execute immediately
                    app.input.clear();
                    app.input_cursor = 0;
                    app.autocomplete = None;
                    Some(dispatch_command(cmd.name, None))
                }
            } else {
                app.autocomplete = None;
                Some(InputAction::None)
            }
        }
        KeyCode::Esc => {
            app.autocomplete = None;
            Some(InputAction::None)
        }
        _ => None, // fall through to normal handling
    }
}

/// Update the autocomplete state based on current input content.
pub(super) fn update_autocomplete(app: &mut App) {
    let trimmed = app.input.trim_start();
    if trimmed.starts_with('/') {
        // Only show autocomplete when we're still in the "command part"
        // (no space yet, unless it's a command with args being typed)
        let first_space = trimmed.find(' ');
        if first_space.is_none() {
            // Still typing the command name — filter
            let matches = filter_commands(trimmed);
            if matches.is_empty() {
                app.autocomplete = None;
            } else {
                let prev_selected = app
                    .autocomplete
                    .as_ref()
                    .map(|ac| ac.selected)
                    .unwrap_or(0);
                app.autocomplete = Some(AutocompleteState {
                    selected: prev_selected.min(matches.len().saturating_sub(1)),
                    matches,
                });
            }
        } else {
            // User has typed a space — command part done, close autocomplete
            app.autocomplete = None;
        }
    } else {
        app.autocomplete = None;
    }
}
