//! State for the `/mcp` sub-page — MCP server status list with action menu.

use loopal_protocol::McpServerSnapshot;

/// Full state for the MCP status sub-page.
pub struct McpPageState {
    pub servers: Vec<McpServerSnapshot>,
    pub selected: usize,
    pub scroll_offset: usize,
    /// `false` until the first McpStatusReport event has been received.
    pub loaded: bool,
    /// When set, an action menu is open for the selected server.
    pub action_menu: Option<ActionMenu>,
}

/// Inline action menu for an MCP server.
pub struct ActionMenu {
    pub server_name: String,
    pub options: Vec<McpAction>,
    pub cursor: usize,
}

/// Actions available in the MCP server action menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpAction {
    Reconnect,
    Disconnect,
}

impl McpAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Reconnect => "Reconnect",
            Self::Disconnect => "Disconnect",
        }
    }
}

impl McpPageState {
    pub fn new(servers: Option<Vec<McpServerSnapshot>>) -> Self {
        let loaded = servers.is_some();
        Self {
            servers: servers.unwrap_or_default(),
            selected: 0,
            scroll_offset: 0,
            loaded,
            action_menu: None,
        }
    }

    pub fn selected_server(&self) -> Option<&McpServerSnapshot> {
        self.servers.get(self.selected)
    }

    pub fn open_action_menu(&mut self) {
        let Some(server) = self.selected_server() else {
            return;
        };
        let name = server.name.clone();
        let is_connected = server.status == "connected";
        let mut options = vec![McpAction::Reconnect];
        if is_connected {
            options.insert(0, McpAction::Disconnect);
        }
        self.action_menu = Some(ActionMenu {
            server_name: name,
            options,
            cursor: 0,
        });
    }
}

impl ActionMenu {
    pub fn cursor_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn cursor_down(&mut self) {
        if self.cursor + 1 < self.options.len() {
            self.cursor += 1;
        }
    }

    pub fn selected_action(&self) -> Option<McpAction> {
        self.options.get(self.cursor).copied()
    }
}
