pub mod backend;
pub mod backend_types;
pub mod memory_channel;
pub mod output_tail;
pub mod permission;
mod tool;
pub mod truncate;

pub use backend::{Backend, ExecOutcome};
pub use backend_types::{
    EditResult, ExecResult, FetchResult, FileInfo, FileMatchResult, GlobEntry, GlobOptions,
    GlobSearchResult, GrepOptions, GrepSearchResult, LsEntry, LsResult, MatchGroup, MatchLine,
    ReadResult, TimeoutSecs, WriteResult,
};
pub use memory_channel::MemoryChannel;
pub use output_tail::OutputTail;
pub use permission::{PermissionDecision, PermissionLevel, PermissionMode};
pub use tool::{Tool, ToolContext, ToolDefinition, ToolDispatch, ToolResult};
pub use truncate::{
    DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_OUTPUT_LINES, OverflowResult, handle_overflow,
    needs_truncation, save_to_overflow_file, truncate_output,
};
