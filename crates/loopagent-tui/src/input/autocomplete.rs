use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AutocompleteState};
use crate::command::filter_entries;

use super::InputAction;
use super::commands::{dispatch_command, expand_skill};

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
            let selected_idx = app
                .autocomplete
                .as_ref()
                .and_then(|ac| ac.matches.get(ac.selected).copied());
            if let Some(idx) = selected_idx {
                let entry = &app.commands[idx];
                let suffix = if entry.has_arg { " " } else { "" };
                let new_input = format!("{}{suffix}", entry.name);
                app.input_cursor = new_input.len();
                app.input = new_input;
            }
            app.autocomplete = None;
            Some(InputAction::None)
        }
        KeyCode::Enter => {
            // Enter = execute the selected command
            let selected_idx = app
                .autocomplete
                .as_ref()
                .and_then(|ac| ac.matches.get(ac.selected).copied());
            if let Some(idx) = selected_idx {
                let entry = &app.commands[idx];
                if entry.has_arg {
                    // Needs argument: fill command name + space, wait for input
                    let new_input = format!("{} ", entry.name);
                    app.input_cursor = new_input.len();
                    app.input = new_input;
                    app.autocomplete = None;
                    Some(InputAction::None)
                } else if let Some(ref body) = entry.skill_body {
                    // No-arg skill: expand and push to inbox
                    let expanded = expand_skill(body, "");
                    app.input.clear();
                    app.input_cursor = 0;
                    app.autocomplete = None;
                    Some(InputAction::InboxPush(expanded))
                } else {
                    // Built-in command: dispatch immediately
                    let name = entry.name.clone();
                    app.input.clear();
                    app.input_cursor = 0;
                    app.autocomplete = None;
                    Some(dispatch_command(&name, None))
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
            let matches = filter_entries(&app.commands, &trimmed);
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
