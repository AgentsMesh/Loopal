//! Key handler for the MCP server status sub-page.

use crossterm::event::{KeyCode, KeyEvent};

use super::InputAction;
use crate::app::{App, SubPage};

pub(super) fn handle_mcp_page_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let state = match app.sub_page.as_mut() {
        Some(SubPage::McpPage(s)) => s,
        _ => return InputAction::None,
    };

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
            if let Some(server) = state.selected_server() {
                let name = server.name.clone();
                return InputAction::McpReconnect(name);
            }
            InputAction::None
        }
        _ => InputAction::None,
    }
}
