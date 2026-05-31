mod errors;
mod helpers;
pub mod io_error;
pub mod memory_graph_error;

pub use errors::{
    AgentOutput, ConfigError, HookError, LoopalError, McpError, ProviderError, StorageError,
    TerminateReason, ToolError,
};
pub use io_error::{ProcessHandle, ToolIoError};
pub use memory_graph_error::{MemoryGraphError, MemorySubsystemBootstrapError};

pub type Result<T> = std::result::Result<T, LoopalError>;
