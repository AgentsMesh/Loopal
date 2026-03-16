use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoopAgentError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),

    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Permission denied: {0}")]
    Permission(String),

    #[error("Hook error: {0}")]
    Hook(#[from] HookError),

    #[error("MCP error: {0}")]
    Mcp(#[from] McpError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("SSE parse error: {0}")]
    SseParse(String),

    #[error("API error: status={status}, message={message}")]
    Api { status: u16, message: String },

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Rate limited: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("Stream ended unexpectedly")]
    StreamEnded,
}

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Timeout after {0}ms")]
    Timeout(u64),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid value for {field}: {reason}")]
    InvalidValue { field: String, reason: String },
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Could not determine home directory")]
    HomeDirNotFound,
}

#[derive(Debug, Error)]
pub enum HookError {
    #[error("Hook execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Hook timeout: {0}")]
    Timeout(String),

    #[error("Hook rejected: {0}")]
    Rejected(String),
}

#[derive(Debug, Error)]
pub enum McpError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Server not found: {0}")]
    ServerNotFound(String),
}

pub type Result<T> = std::result::Result<T, LoopAgentError>;

impl ProviderError {
    /// Check if this is a rate limit error
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, ProviderError::RateLimited { .. })
    }

    /// Get the retry-after duration in milliseconds, if this is a rate limit error
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            ProviderError::RateLimited { retry_after_ms } => Some(*retry_after_ms),
            _ => None,
        }
    }
}

impl LoopAgentError {
    /// Check if this is a rate limit error
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, LoopAgentError::Provider(ProviderError::RateLimited { .. }))
    }

    /// Get the retry-after duration in milliseconds, if this is a rate limit error
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            LoopAgentError::Provider(ProviderError::RateLimited { retry_after_ms }) => {
                Some(*retry_after_ms)
            }
            _ => None,
        }
    }
}
