use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AutocompleteState};
use crate::command::filter_entries;

use super::InputAction;

/// Handle keys when the autocomplete menu is open.
/// Returns `Some(action)` if the key was consumed, `None` to fall through.
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
        KeyCode::Tab => {
            // Tab = autocomplete only: fill command name into input, never execute
            let entry = app
                .autocomplete
                .as_ref()
                .and_then(|ac| ac.matches.get(ac.selected));
            if let Some(entry) = entry {
                let suffix = if entry.has_arg { " " } else { "" };
                let new_input = format!("{}{suffix}", entry.name);
                app.input_cursor = new_input.len();
                app.input = new_input;
            }
            app.autocomplete = None;
            Some(InputAction::None)
        }
        KeyCode::Enter => {
            // Enter = execute the selected command via unified RunCommand
            let entry = app
                .autocomplete
                .as_ref()
                .and_then(|ac| ac.matches.get(ac.selected))
                .cloned();
            if let Some(entry) = entry {
                if entry.has_arg {
                    // Needs argument: fill command name + space, wait for input
                    let new_input = format!("{} ", entry.name);
                    app.input_cursor = new_input.len();
                    app.input = new_input;
                    app.autocomplete = None;
                    Some(InputAction::None)
                } else {
                    // Execute immediately via registry
                    let name = entry.name;
                    app.input.clear();
                    app.input_cursor = 0;
                    app.autocomplete = None;
                    Some(InputAction::RunCommand(name, None))
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
///
/// When a `/` prefix is detected, skills are reloaded from disk so that
/// newly created skill files are available without restarting.
pub(super) fn update_autocomplete(app: &mut App) {
    let trimmed = app.input.trim_start().to_string();
    if trimmed.starts_with('/') {
        // Only show autocomplete when still typing the command name (no space yet)
        let first_space = trimmed.find(' ');
        if first_space.is_none() {
            // Refresh commands from disk so new/changed skills appear immediately
            app.refresh_commands();
            let entries = app.command_registry.entries();
            let matches = filter_entries(&entries, &trimmed);
            if matches.is_empty() {
                app.autocomplete = None;
            } else {
                let prev = app.autocomplete.as_ref().map(|ac| ac.selected).unwrap_or(0);
                app.autocomplete = Some(AutocompleteState {
                    selected: prev.min(matches.len().saturating_sub(1)),
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
