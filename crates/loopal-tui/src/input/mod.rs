mod actions;
mod autocomplete;
mod commands;
mod editing;
mod modal;
pub(crate) mod multiline;
mod navigation;
pub(crate) mod paste;
mod sub_page;

pub use actions::*;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, FocusMode, PanelKind};
use autocomplete::{handle_autocomplete_key, update_autocomplete};
use editing::{handle_backspace, handle_ctrl_c, handle_enter};
use navigation::{
    DEFAULT_WRAP_WIDTH, handle_down, handle_esc, handle_up, move_cursor_left, move_cursor_right,
};

/// Process a key event and update the app's input state.
pub fn handle_key(app: &mut App, key: KeyEvent) -> InputAction {
    if let Some(action) = modal::handle_modal_keys(app, &key) {
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

/// Handle global shortcuts: Ctrl combos, Shift+Tab.
fn handle_global_keys(app: &mut App, key: &KeyEvent) -> Option<InputAction> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') => return Some(handle_ctrl_c(app)),
            KeyCode::Char('d') => return Some(InputAction::Quit),
            KeyCode::Char('v') => return Some(InputAction::PasteRequested),
            // Ctrl+P/N: mode-aware up/down (panel nav in Panel, history in Input)
            KeyCode::Char('p') if matches!(app.focus_mode, FocusMode::Panel(_)) => {
                return Some(InputAction::PanelUp);
            }
            KeyCode::Char('n') if matches!(app.focus_mode, FocusMode::Panel(_)) => {
                return Some(InputAction::PanelDown);
            }
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

/// Handle normal input keys — dispatch by current focus mode.
fn handle_normal_key(app: &mut App, key: &KeyEvent) -> InputAction {
    match app.focus_mode {
        FocusMode::Panel(_) => handle_panel_key(app, key),
        FocusMode::Input => handle_input_mode_key(app, key),
    }
}

/// Keys in Panel mode: Up/Down navigate, Enter drills in (agents), Tab switches/cycles.
fn handle_panel_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let kind = match app.focus_mode {
        FocusMode::Panel(k) => k,
        _ => return InputAction::None,
    };
    match key.code {
        KeyCode::Up => InputAction::PanelUp,
        KeyCode::Down => InputAction::PanelDown,
        KeyCode::Enter if kind == PanelKind::Agents => InputAction::EnterAgentView,
        KeyCode::Delete if kind == PanelKind::Agents => InputAction::TerminateFocusedAgent,
        KeyCode::Tab => InputAction::PanelTab,
        KeyCode::Esc => InputAction::ExitPanel,
        KeyCode::Char(c) => {
            // Auto-switch to Input mode and insert the character
            app.focus_mode = FocusMode::Input;
            app.input.insert(app.input_cursor, c);
            app.input_cursor += c.len_utf8();
            InputAction::None
        }
        KeyCode::Backspace => {
            // Auto-switch to Input mode and delete
            app.focus_mode = FocusMode::Input;
            handle_backspace(app)
        }
        _ => InputAction::None,
    }
}

/// Keys in Input mode: typing, navigation, submit.
fn handle_input_mode_key(app: &mut App, key: &KeyEvent) -> InputAction {
    // Auto-scroll to bottom on input interaction (except scroll/panel/escape keys)
    if !matches!(
        key.code,
        KeyCode::PageUp | KeyCode::PageDown | KeyCode::Tab | KeyCode::Esc
    ) {
        app.scroll_offset = 0;
    }
    match key.code {
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
        KeyCode::Up => handle_up(app),
        KeyCode::Down => handle_down(app),
        KeyCode::Tab => InputAction::EnterPanel,
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
