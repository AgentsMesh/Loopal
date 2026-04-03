use std::time::Duration;

use loopal_tool_api::{DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_OUTPUT_LINES};

/// Resource limits applied by `LocalBackend`.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum file size in bytes that `read()` will accept.
    pub max_file_read_bytes: u64,
    /// Maximum lines in command output.
    pub max_output_lines: usize,
    /// Maximum bytes in command output.
    pub max_output_bytes: usize,
    /// Cap on glob result count before truncation.
    pub max_glob_results: usize,
    /// Cap on grep match count before truncation.
    pub max_grep_matches: usize,
    /// Maximum HTTP response body size in bytes.
    pub max_fetch_bytes: usize,
    /// Default shell command timeout.
    pub default_timeout: Duration,
    /// HTTP fetch timeout.
    pub fetch_timeout: Duration,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_file_read_bytes: 10 * 1024 * 1024, // 10 MB
            max_output_lines: DEFAULT_MAX_OUTPUT_LINES,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            max_glob_results: 10_000,
            max_grep_matches: 500,
            max_fetch_bytes: 5 * 1024 * 1024,          // 5 MB
            default_timeout: Duration::from_secs(300), // 5 min
            fetch_timeout: Duration::from_secs(30),
        }
    }
}
