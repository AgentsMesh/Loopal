use loopal_protocol::McpServerSnapshot;

pub const ROOT_AGENT: &str = "main";

pub struct SessionState {
    pub active_view: String,
    /// UI-local preference for the next `ThinkingSwitch`. Not synced
    /// from agent events — purely client-side.
    pub thinking_config: String,
    pub root_session_id: Option<String>,
    pub mcp_status: Option<Vec<McpServerSnapshot>>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            active_view: ROOT_AGENT.to_string(),
            thinking_config: "auto".to_string(),
            root_session_id: None,
            mcp_status: None,
        }
    }
}
