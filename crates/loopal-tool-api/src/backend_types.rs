//! Value types returned by [`Backend`](super::Backend) methods.
//!
//! These are deliberately simple structs so that tool crates only depend
//! on `loopal-tool-api` (a leaf crate) for their I/O interface.

/// Result of a file read operation.
#[derive(Debug, Clone)]
pub struct ReadResult {
    /// File content (with line-numbered formatting if applicable).
    pub content: String,
    /// Total number of lines in the original file.
    pub total_lines: usize,
}

/// Result of a file write operation.
#[derive(Debug, Clone)]
pub struct WriteResult {
    pub bytes_written: usize,
}

/// Result of a search-and-replace edit operation.
#[derive(Debug, Clone)]
pub struct EditResult {
    /// Number of replacements made.
    pub replacements: usize,
}

/// Result of a shell command execution.
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Result of an HTTP fetch operation.
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub body: String,
    pub content_type: Option<String>,
    pub status: u16,
}

/// Metadata about a single file or directory.
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub size: u64,
    pub is_dir: bool,
    pub is_binary: bool,
    pub modified: Option<u64>,
}

/// Result of a directory listing.
#[derive(Debug, Clone)]
pub struct LsResult {
    pub entries: Vec<LsEntry>,
}

/// Single entry in a directory listing.
#[derive(Debug, Clone)]
pub struct LsEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<u64>,
    pub permissions: Option<u32>,
}

// --- Glob search types ---

/// Options for a glob file search.
#[derive(Debug, Clone)]
pub struct GlobOptions {
    pub pattern: String,
    pub path: Option<String>,
    pub type_filter: Option<String>,
    pub max_results: usize,
}

/// Result of a glob search.
#[derive(Debug, Clone)]
pub struct GlobSearchResult {
    pub entries: Vec<GlobEntry>,
    /// `true` when results were capped at the configured limit.
    pub truncated: bool,
}

/// Single entry in a glob search result.
#[derive(Debug, Clone)]
pub struct GlobEntry {
    pub path: String,
    pub modified_secs: Option<u64>,
}

// --- Grep search types ---

/// Options for a regex content search.
#[derive(Debug, Clone)]
pub struct GrepOptions {
    pub pattern: String,
    pub path: Option<String>,
    pub glob_filter: Option<String>,
    pub case_insensitive: bool,
    pub multiline: bool,
    pub fixed_strings: bool,
    pub context_before: usize,
    pub context_after: usize,
    pub type_filter: Option<String>,
    pub max_matches: usize,
}

/// Result of a grep content search.
#[derive(Debug, Clone)]
pub struct GrepSearchResult {
    pub file_matches: Vec<FileMatchResult>,
    pub total_match_count: usize,
}

/// All matches within a single file.
#[derive(Debug, Clone)]
pub struct FileMatchResult {
    pub path: String,
    pub groups: Vec<MatchGroup>,
}

/// A group of contiguous lines (matches + surrounding context).
#[derive(Debug, Clone)]
pub struct MatchGroup {
    pub lines: Vec<MatchLine>,
}

/// A single line in a match group — either a match or a context line.
#[derive(Debug, Clone)]
pub struct MatchLine {
    pub line_num: usize,
    pub content: String,
    pub is_match: bool,
}
