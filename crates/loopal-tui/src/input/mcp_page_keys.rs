//! Key handler for the MCP server status sub-page.

use crossterm::event::{KeyCode, KeyEvent};

use super::InputAction;
use crate::app::McpAction;
use crate::app::{App, SubPage};

pub(super) fn handle_mcp_page_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let state = match app.sub_page.as_mut() {
        Some(SubPage::McpPage(s)) => s,
        _ => return InputAction::None,
    };

    if state.action_menu.is_some() {
        return handle_menu_key(app, key);
    }

    match key.code {
        KeyCode::Esc => {
            app.sub_page = None;
            app.last_esc_time = None;
            InputAction::None
        }
        KeyCode::Up => {
            state.selected = state.selected.saturating_sub(1);
            InputAction::None
        }
        KeyCode::Down => {
            if !state.servers.is_empty() && state.selected + 1 < state.servers.len() {
                state.selected += 1;
            }
            InputAction::None
        }
        KeyCode::Enter => {
            state.open_action_menu();
            InputAction::None
        }
        _ => InputAction::None,
    }
}

fn handle_menu_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let state = match app.sub_page.as_mut() {
        Some(SubPage::McpPage(s)) => s,
        _ => return InputAction::None,
    };
    let menu = match state.action_menu.as_mut() {
        Some(m) => m,
        None => return InputAction::None,
    };

    match key.code {
        KeyCode::Esc => {
            state.action_menu = None;
            InputAction::None
        }
        KeyCode::Up => {
            menu.cursor_up();
            InputAction::None
        }
        KeyCode::Down => {
            menu.cursor_down();
            InputAction::None
        }
        KeyCode::Enter => {
            let action = menu.selected_action();
            let name = menu.server_name.clone();
            state.action_menu = None;
            match action {
                Some(McpAction::Reconnect) => InputAction::McpReconnect(name),
                Some(McpAction::Disconnect) => InputAction::McpDisconnect(name),
                None => InputAction::None,
            }
        }
        _ => InputAction::None,
    }
}
