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

    #[error("Context overflow: {message}")]
    ContextOverflow { message: String },
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

/// Why the agent loop terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminateReason {
    /// Agent completed its task (called AttemptCompletion or natural finish).
    Goal,
    /// LLM or system error.
    Error,
    /// Reached max_turns limit.
    MaxTurns,
    /// Cancelled by parent or user.
    Aborted,
}

/// Structured output from an agent loop execution.
#[derive(Debug, Clone)]
pub struct AgentOutput {
    /// Best-effort result text (may be non-empty even on Error/MaxTurns).
    pub result: String,
    /// Why the loop stopped.
    pub terminate_reason: TerminateReason,
}

impl ProviderError {
    /// Check if this is a rate limit error
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, ProviderError::RateLimited { .. })
    }

    /// Check if this error is retryable (rate limit, server errors, etc.)
    pub fn is_retryable(&self) -> bool {
        match self {
            ProviderError::RateLimited { .. } => true,
            ProviderError::Api { status, message } => {
                // 400 with context overflow keywords is deterministic — never retryable
                if *status == 400
                    && (message.contains("invalid_request_error")
                        || message.contains("prompt is too long")
                        || message.contains("maximum context length"))
                {
                    return false;
                }
                matches!(status, 429 | 500 | 502 | 503 | 529)
            }
            ProviderError::ContextOverflow { .. } => false,
            _ => false,
        }
    }

    /// Check if this error indicates the prompt exceeded the model's context window.
    pub fn is_context_overflow(&self) -> bool {
        match self {
            ProviderError::ContextOverflow { .. } => true,
            ProviderError::Api { status, message } if *status == 400 => {
                message.contains("prompt is too long")
                    || message.contains("maximum context length")
            }
            _ => false,
        }
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

    /// Check if this error is retryable (rate limit, server errors, etc.)
    pub fn is_retryable(&self) -> bool {
        matches!(self, LoopAgentError::Provider(e) if e.is_retryable())
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

    /// Check if this error indicates the prompt exceeded the model's context window.
    pub fn is_context_overflow(&self) -> bool {
        matches!(self, LoopAgentError::Provider(e) if e.is_context_overflow())
    }
}
