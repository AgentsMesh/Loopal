pub mod backend;
pub mod backend_types;
pub mod memory_channel;
pub mod output_tail;
pub mod permission;
pub mod provider_resolver;
mod tool;
mod tool_context;
pub mod truncate;
pub mod truncate_middle;

pub use backend::{Backend, ExecOutcome};
pub use backend_types::{
    EditResult, ExecResult, FetchResult, FileInfo, FileMatchResult, GlobEntry, GlobOptions,
    GlobSearchResult, GrepOptions, GrepSearchResult, LsEntry, LsResult, MatchGroup, MatchLine,
    ReadResult, TimeoutSecs, WriteResult,
};
pub use memory_channel::MemoryChannel;
pub use output_tail::OutputTail;
pub use permission::{PermissionDecision, PermissionLevel, PermissionMode};
pub use provider_resolver::{FetchRefinerPolicy, OneShotChatError, OneShotChatService};
pub use tool::{Tool, ToolDefinition, ToolDispatch, ToolResult};
pub use tool_context::ToolContext;
pub use truncate::{
    DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_OUTPUT_LINES, OverflowResult, extract_overflow_path,
    handle_overflow, humanize_size, needs_truncation, save_to_overflow_file, truncate_output,
    truncate_tail,
};
pub use truncate_middle::truncate_middle;
