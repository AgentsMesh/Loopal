//! Navigation key handlers: cursor movement, history, up/down/esc.

use std::time::Instant;

use super::InputAction;
use super::multiline;
use crate::app::{App, FocusMode};

/// Default wrap width when terminal width is unknown.
pub(super) const DEFAULT_WRAP_WIDTH: usize = 80;

pub(super) fn move_cursor_left(app: &mut App) {
    if app.input_cursor > 0 {
        app.input_cursor = app.input[..app.input_cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
    }
}

pub(super) fn move_cursor_right(app: &mut App) {
    if app.input_cursor < app.input.len() {
        app.input_cursor = app.input[app.input_cursor..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| app.input_cursor + i)
            .unwrap_or(app.input.len());
    }
}

/// Up: multiline navigation first, then history browse.
pub(super) fn handle_up(app: &mut App) -> InputAction {
    app.scroll_offset = 0;
    if multiline::is_multiline(&app.input, DEFAULT_WRAP_WIDTH)
        && let Some(new_cursor) =
            multiline::cursor_up(&app.input, app.input_cursor, DEFAULT_WRAP_WIDTH)
    {
        app.input_cursor = new_cursor;
        return InputAction::None;
    }
    // Fall back to history browse
    if !app.input_history.is_empty() {
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

/// Down: multiline navigation first, then history browse.
pub(super) fn handle_down(app: &mut App) -> InputAction {
    app.scroll_offset = 0;
    if multiline::is_multiline(&app.input, DEFAULT_WRAP_WIDTH)
        && let Some(new_cursor) =
            multiline::cursor_down(&app.input, app.input_cursor, DEFAULT_WRAP_WIDTH)
    {
        app.input_cursor = new_cursor;
        return InputAction::None;
    }
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

pub(super) fn handle_esc(app: &mut App) -> InputAction {
    // Panel mode: exit back to Input (don't trigger view exit or rewind)
    if matches!(app.focus_mode, FocusMode::Panel(_)) {
        return InputAction::ExitPanel;
    }
    // Priority 1: exit agent view
    let active_view = app.session.lock().active_view.clone();
    if active_view != loopal_session::ROOT_AGENT {
        tracing::info!(view = %active_view, "ESC: exit agent view (not root)");
        return InputAction::ExitAgentView;
    }
    let is_idle = app.session.lock().is_active_agent_idle();
    if !is_idle {
        tracing::info!("ESC: agent busy, sending interrupt");
        return InputAction::Interrupt;
    }
    tracing::debug!("ESC: agent idle, no interrupt");
    let now = Instant::now();
    let is_empty = app.input.is_empty();
    if is_empty {
        if let Some(last) = app.last_esc_time.take()
            && now.duration_since(last).as_millis() < 300
        {
            return InputAction::RunCommand("/rewind".to_string(), None);
        }
        app.last_esc_time = Some(now);
    }
    InputAction::None
}
