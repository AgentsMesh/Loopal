use std::time::Duration;

use crate::path::ResolvedPath;

#[derive(Debug, Clone)]
pub struct ReadResult {
    pub content: String,
    pub total_lines: usize,
}

#[derive(Debug, Clone)]
pub struct ImageResult {
    pub media_type: String,
    pub data: String,
    pub dimensions: (u32, u32),
    pub byte_size: usize,
}

#[derive(Debug, Clone)]
pub struct WriteResult {
    pub bytes_written: usize,
}

// stdout = HeadTail preview (short → full; long → head + tail + elision marker).
// stderr = ≤8 KB capped buffer (front-first trim). log_path is the merged tmp
// file with full interleaved output (stderr lines prefixed `[err] `) — Read
// it if complete output is needed.
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub exit_code: i32,
    pub log_path: std::path::PathBuf,
}

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub body: String,
    pub content_type: Option<String>,
    pub status: u16,
    pub overflow_path: Option<String>,
    // Populated only when redirect host differs from the requested URL —
    // signals to caller that following is a cross-origin decision.
    pub final_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub size: u64,
    pub is_dir: bool,
    pub is_binary: bool,
    pub modified: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct LsResult {
    pub entries: Vec<LsEntry>,
}

#[derive(Debug, Clone)]
pub struct LsEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<u64>,
    pub permissions: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct GlobOptions {
    pub pattern: String,
    pub path: Option<ResolvedPath>,
    pub type_filter: Option<String>,
    pub max_results: usize,
}

#[derive(Debug, Clone)]
pub struct GlobSearchResult {
    pub entries: Vec<GlobEntry>,
    pub truncated: bool,
    pub timed_out: bool,
    pub overflow_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GlobEntry {
    pub path: String,
    pub modified_secs: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct GrepOptions {
    pub pattern: String,
    pub path: Option<ResolvedPath>,
    pub glob_filter: Option<String>,
    pub case_insensitive: bool,
    pub multiline: bool,
    pub fixed_strings: bool,
    pub context_before: usize,
    pub context_after: usize,
    pub type_filter: Option<String>,
    pub max_matches: usize,
}

#[derive(Debug, Clone)]
pub struct GrepSearchResult {
    pub file_matches: Vec<FileMatchResult>,
    pub total_match_count: usize,
    pub timed_out: bool,
    pub overflow_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileMatchResult {
    pub path: String,
    pub groups: Vec<MatchGroup>,
}

#[derive(Debug, Clone)]
pub struct MatchGroup {
    pub lines: Vec<MatchLine>,
}

#[derive(Debug, Clone)]
pub struct MatchLine {
    pub line_num: usize,
    pub content: String,
    pub is_match: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeoutSecs(u64);

impl TimeoutSecs {
    pub const fn new(secs: u64) -> Self {
        Self(secs)
    }

    pub fn from_tool_input(input: &serde_json::Value, default_secs: u64) -> Self {
        Self(input["timeout"].as_u64().unwrap_or(default_secs))
    }

    pub const fn as_secs(&self) -> u64 {
        self.0
    }

    pub const fn to_duration(&self) -> Duration {
        Duration::from_secs(self.0)
    }

    pub fn to_duration_clamped(&self, max: Duration) -> Duration {
        let d = Duration::from_secs(self.0);
        if d > max { max } else { d }
    }
}

impl std::fmt::Display for TimeoutSecs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}s", self.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct EnvOverride {
    pub vars: std::collections::HashMap<String, String>,
}

impl EnvOverride {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.insert(key.into(), value.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}
