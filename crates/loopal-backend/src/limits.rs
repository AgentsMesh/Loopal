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
    /// Default shell command timeout (ms).
    pub default_timeout_ms: u64,
    /// HTTP fetch timeout (seconds).
    pub fetch_timeout_secs: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_file_read_bytes: 10 * 1024 * 1024, // 10 MB
            max_output_lines: 2_000,
            max_output_bytes: 512_000,
            max_glob_results: 10_000,
            max_grep_matches: 500,
            max_fetch_bytes: 5 * 1024 * 1024, // 5 MB
            default_timeout_ms: 300_000,      // 5 min
            fetch_timeout_secs: 30,
        }
    }
}
