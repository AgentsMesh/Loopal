//! State for the `/mcp` sub-page — MCP server status list.

use loopal_protocol::McpServerSnapshot;

/// Full state for the MCP status sub-page.
pub struct McpPageState {
    pub servers: Vec<McpServerSnapshot>,
    pub selected: usize,
    pub scroll_offset: usize,
    /// `false` until the first McpStatusReport event has been received.
    pub loaded: bool,
}

impl McpPageState {
    pub fn new(servers: Option<Vec<McpServerSnapshot>>) -> Self {
        let loaded = servers.is_some();
        Self {
            servers: servers.unwrap_or_default(),
            selected: 0,
            scroll_offset: 0,
            loaded,
        }
    }

    pub fn selected_server(&self) -> Option<&McpServerSnapshot> {
        self.servers.get(self.selected)
    }
}
