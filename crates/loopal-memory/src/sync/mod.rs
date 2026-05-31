pub mod init;
pub mod persist;
pub mod subsystem;
pub mod watcher;

use std::path::{Path, PathBuf};

pub use init::{InitStats, scan_directory};
pub use loopal_error::MemorySubsystemBootstrapError;
pub use persist::{persist_extraction, relative_path};
pub use subsystem::MemorySubsystem;
pub use watcher::{WatcherHandle, watch};

pub const MEMORY_INDEX_FILE_NAME: &str = "MEMORY.md";

pub fn memory_index_path(memory_dir: &Path) -> PathBuf {
    memory_dir.join(MEMORY_INDEX_FILE_NAME)
}

pub fn is_indexable_md(path: &Path) -> bool {
    if path.extension().and_then(|s| s.to_str()) != Some("md") {
        return false;
    }
    if let Some(name) = path.file_name().and_then(|s| s.to_str())
        && name.eq_ignore_ascii_case(MEMORY_INDEX_FILE_NAME)
    {
        return false;
    }
    true
}
