//! Connection status and shared types for the MCP module.

/// Lifecycle state of a single MCP server connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Not connected (initial state or after clean disconnect).
    Disconnected,
    /// Connection attempt in progress.
    Connecting,
    /// Successfully connected and capabilities discovered.
    Connected,
    /// Permanently failed after exhausting retries. Requires manual restart.
    Failed(String),
}

impl ConnectionStatus {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "disconnected"),
            Self::Connecting => write!(f, "connecting"),
            Self::Connected => write!(f, "connected"),
            Self::Failed(reason) => write!(f, "failed: {reason}"),
        }
    }
}

/// Summary of which MCP capabilities the server supports.
/// Extracted from the server's `InitializeResult` after handshake.
#[derive(Debug, Clone, Default)]
pub struct CapabilitySummary {
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
}

/// An MCP resource exposed by a server.
#[derive(Debug, Clone)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// An MCP prompt template exposed by a server.
#[derive(Debug, Clone)]
pub struct McpPrompt {
    pub name: String,
    pub description: Option<String>,
}
