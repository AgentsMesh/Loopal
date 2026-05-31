pub mod agent_output;
pub mod consolidation;
pub mod date;
pub mod event_log;
pub mod extract;
pub mod graph;
pub mod init;
mod observer;
pub mod policy;
pub mod render;
pub mod store;
pub mod sync;
pub mod synthesize;
mod tool;
pub mod tools;

pub const PROJECT_MEMORY_DIR: &str = ".loopal/memory";
pub const PROJECT_MEMORY_EVENTS_DIR: &str = ".loopal/memory-events";

pub use event_log::{
    Event, EventKind, EventLogWriter, GcStats, RecallSource, RecallStats, RecallStatsMap,
    fold_events, run_gc,
};
pub use extract::{ExtractionResult, UnresolvedLink, extract_file, slug_from_path};
pub use graph::{RecallParams, RecallResult, format_recall, recall};
pub use init::ensure_gitignore;
pub use observer::{
    MEMORY_AGENT_PROMPT, MEMORY_CONSOLIDATION_PROMPT, MemoryObserver, MemoryProcessor,
};
pub use render::render_memory_md;
pub use store::queries_files::FileCacheEntry;
pub use store::{
    BfsConfig, EdgeKind, FtsHit, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance,
    Subgraph,
};
pub use sync::{
    InitStats, MEMORY_INDEX_FILE_NAME, MemorySubsystem, MemorySubsystemBootstrapError,
    WatcherHandle, memory_index_path, scan_directory, watch,
};
pub use synthesize::{SynthesisStats, run_all as synthesize_all};
pub use tool::MemoryTool;
pub use tools::{MemoryImportanceTool, MemoryRecallTool};
