use crate::app::{App, FocusMode};

use super::InputAction;
use super::commands::try_execute_slash_command;

pub(super) fn handle_enter(app: &mut App) -> InputAction {
    // Empty Enter with a focused sub-agent → drill into its view
    if app.input.is_empty()
        && app.pending_images.is_empty()
        && let Some(ref focused) = app.focused_agent
    {
        let active = &app.session.lock().active_view;
        if focused != active {
            return InputAction::EnterAgentView;
        }
    }
    let trimmed = app.input.trim().to_string();
    if trimmed.starts_with('/') {
        app.refresh_commands();
    }
    if let Some(cmd_action) = try_execute_slash_command(&trimmed, &app.command_registry.entries()) {
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

pub(super) fn handle_backspace(app: &mut App) -> InputAction {
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

/// Ctrl+C: clear input → exit Panel → clear focus → interrupt agent.
pub(super) fn handle_ctrl_c(app: &mut App) -> InputAction {
    if !app.input.is_empty() || !app.pending_images.is_empty() {
        app.input.clear();
        app.input_cursor = 0;
        app.history_index = None;
        app.pending_images.clear();
        app.paste_map.clear();
        app.autocomplete = None;
        InputAction::None
    } else if matches!(app.focus_mode, FocusMode::Panel(_)) {
        app.focused_agent = None;
        app.focused_bg_task = None;
        app.focus_mode = FocusMode::Input;
        app.agent_panel_offset = 0;
        InputAction::None
    } else if app.focused_agent.is_some() {
        app.focused_agent = None;
        InputAction::None
    } else if !app.session.lock().is_active_agent_idle() {
        InputAction::Interrupt
    } else {
        InputAction::None
    }
}
